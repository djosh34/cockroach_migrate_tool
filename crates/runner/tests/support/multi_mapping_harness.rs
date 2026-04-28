use std::{
    cell::RefCell,
    fs::{self, File},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::MutexGuard,
    thread,
    time::{Duration, Instant},
};

use ingest_contract::MappingIngestPath;
use tempfile::TempDir;

use crate::e2e_harness::{
    encode_ca_cert_query_value, https_client, investigation_ca_cert_path,
    investigation_server_cert_path, investigation_server_key_path, lock_e2e_database_resources,
    pick_unused_port, read_file, run_audited_cockroach_sql, wait_for_runner_health,
    write_cockroach_wrapper_script, LocalDatabaseEnvironment,
};
use crate::e2e_integrity::VerifyCorrectnessAudit;
use crate::verify_image_harness_support::{VerifyImageHarness, VerifyImageRun};

const CHANGEFEED_RESOLVED_INTERVAL: &str = "1s";

const APP_A_SOURCE_SCHEMA_SQL: &str = r#"
CREATE DATABASE demo_a;
USE demo_a;
CREATE TABLE public.customers (
    id INT8 PRIMARY KEY,
    email STRING NOT NULL
);
CREATE TABLE public.order_items (
    order_id INT8 NOT NULL,
    line_id INT8 NOT NULL,
    sku STRING NOT NULL,
    quantity INT8 NOT NULL,
    PRIMARY KEY (order_id, line_id)
);
INSERT INTO public.customers (id, email) VALUES
    (1, 'alice@example.com'),
    (2, 'bob@example.com');
INSERT INTO public.order_items (order_id, line_id, sku, quantity) VALUES
    (100, 1, 'starter-kit', 2),
    (100, 2, 'bonus-widget', 1);
"#;

const APP_A_DESTINATION_SCHEMA_SQL: &str = r#"
CREATE TABLE public.customers (
    id bigint PRIMARY KEY,
    email text NOT NULL
);
CREATE TABLE public.order_items (
    order_id bigint NOT NULL,
    line_id bigint NOT NULL,
    sku text NOT NULL,
    quantity bigint NOT NULL,
    PRIMARY KEY (order_id, line_id)
);
"#;

const APP_B_SOURCE_SCHEMA_SQL: &str = r#"
CREATE DATABASE demo_b;
USE demo_b;
CREATE TABLE public.invoices (
    id INT8 PRIMARY KEY,
    status STRING NOT NULL,
    amount_cents INT8 NOT NULL
);
CREATE TABLE public.invoice_lines (
    invoice_id INT8 NOT NULL,
    line_id INT8 NOT NULL,
    sku STRING NOT NULL,
    quantity INT8 NOT NULL,
    PRIMARY KEY (invoice_id, line_id)
);
INSERT INTO public.invoices (id, status, amount_cents) VALUES
    (5001, 'open', 12000),
    (5002, 'paid', 7600);
INSERT INTO public.invoice_lines (invoice_id, line_id, sku, quantity) VALUES
    (5001, 1, 'starter-kit', 2),
    (5001, 2, 'support-plan', 1),
    (5002, 1, 'renewal', 1);
"#;

const APP_B_DESTINATION_SCHEMA_SQL: &str = r#"
CREATE TABLE public.invoices (
    id bigint PRIMARY KEY,
    status text NOT NULL,
    amount_cents bigint NOT NULL
);
CREATE TABLE public.invoice_lines (
    invoice_id bigint NOT NULL,
    line_id bigint NOT NULL,
    sku text NOT NULL,
    quantity bigint NOT NULL,
    PRIMARY KEY (invoice_id, line_id)
);
"#;

const APP_A_HELPER_SNAPSHOT_SQL: &str = r#"
SELECT string_agg(entry, ',' ORDER BY entry)
FROM (
    SELECT 'c:' || id::text || ':' || email AS entry
    FROM _cockroach_migration_tool."app-a__public__customers"
    UNION ALL
    SELECT 'o:' || order_id::text || '|' || line_id::text || '|' || sku || '|' || quantity::text
    FROM _cockroach_migration_tool."app-a__public__order_items"
) snapshot;
"#;

const APP_B_HELPER_SNAPSHOT_SQL: &str = r#"
SELECT string_agg(entry, ',' ORDER BY entry)
FROM (
    SELECT 'i:' || id::text || ':' || status || ':' || amount_cents::text AS entry
    FROM _cockroach_migration_tool."app-b__public__invoices"
    UNION ALL
    SELECT 'l:' || invoice_id::text || '|' || line_id::text || '|' || sku || '|' || quantity::text
    FROM _cockroach_migration_tool."app-b__public__invoice_lines"
) snapshot;
"#;

const HELPER_TABLES_SQL: &str = r#"
SELECT string_agg(table_name, ',' ORDER BY table_name)
FROM information_schema.tables
WHERE table_schema = '_cockroach_migration_tool';
"#;

const STREAM_STATE_SQL: &str = r#"
SELECT string_agg(mapping_id || ':' || source_database, ',' ORDER BY mapping_id)
FROM _cockroach_migration_tool.stream_state;
"#;

const TABLE_SYNC_STATE_SQL: &str = r#"
SELECT string_agg(
    mapping_id || ':' || source_table_name || ':' || helper_table_name,
    ',' ORDER BY source_table_name
)
FROM _cockroach_migration_tool.table_sync_state;
"#;

const APP_A_INITIAL_SNAPSHOT: &str =
    "c:1:alice@example.com,c:2:bob@example.com,o:100|1|starter-kit|2,o:100|2|bonus-widget|1";
const APP_A_LIVE_SNAPSHOT: &str = "c:1:alice+vip@example.com,c:3:carol@example.com,o:100|1|starter-kit-v2|5,o:101|1|replacement-kit|3";
const APP_B_INITIAL_SNAPSHOT: &str = "i:5001:open:12000,i:5002:paid:7600,l:5001|1|starter-kit|2,l:5001|2|support-plan|1,l:5002|1|renewal|1";
const APP_B_LIVE_SNAPSHOT: &str =
    "i:5001:sent:12500,i:5003:draft:3300,l:5001|1|starter-kit|4,l:5003|1|expansion-pack|2";
const APP_A_HELPER_TABLES: &str =
    "app-a__public__customers,app-a__public__order_items,stream_state,table_sync_state";
const APP_B_HELPER_TABLES: &str =
    "app-b__public__invoice_lines,app-b__public__invoices,stream_state,table_sync_state";
const APP_A_STREAM_STATE: &str = "app-a:demo_a";
const APP_B_STREAM_STATE: &str = "app-b:demo_b";
const APP_A_TABLE_SYNC_STATE: &str = "app-a:public.customers:app-a__public__customers,app-a:public.order_items:app-a__public__order_items";
const APP_B_TABLE_SYNC_STATE: &str = "app-b:public.invoice_lines:app-b__public__invoice_lines,app-b:public.invoices:app-b__public__invoices";

#[derive(Clone, Copy)]
struct MappingSpec {
    id: &'static str,
    source_database: &'static str,
    destination_database: &'static str,
    destination_user: &'static str,
    destination_password: &'static str,
    selected_tables: &'static [&'static str],
    source_schema_sql: &'static str,
    destination_schema_sql: &'static str,
}

const APP_A: MappingSpec = MappingSpec {
    id: "app-a",
    source_database: "demo_a",
    destination_database: "app_a",
    destination_user: "migration_user_a",
    destination_password: "runner-secret-a",
    selected_tables: &["public.customers", "public.order_items"],
    source_schema_sql: APP_A_SOURCE_SCHEMA_SQL,
    destination_schema_sql: APP_A_DESTINATION_SCHEMA_SQL,
};

const APP_B: MappingSpec = MappingSpec {
    id: "app-b",
    source_database: "demo_b",
    destination_database: "app_b",
    destination_user: "migration_user_b",
    destination_password: "runner-secret-b",
    selected_tables: &["public.invoices", "public.invoice_lines"],
    source_schema_sql: APP_B_SOURCE_SCHEMA_SQL,
    destination_schema_sql: APP_B_DESTINATION_SCHEMA_SQL,
};

const MAPPINGS: [MappingSpec; 2] = [APP_A, APP_B];

pub struct MultiMappingHarness {
    _database_test_guard: MutexGuard<'static, ()>,
    databases: LocalDatabaseEnvironment,
    temp_dir: TempDir,
    runner_port: u16,
    runner_config_path: PathBuf,
    wrapper_bin_dir: PathBuf,
    cockroach_wrapper_log_path: PathBuf,
    runner_stdout_path: PathBuf,
    runner_stderr_path: PathBuf,
    runner_process: RefCell<Option<Child>>,
}

impl MultiMappingHarness {
    pub fn start() -> Self {
        let database_test_guard = lock_e2e_database_resources();
        let mut databases = LocalDatabaseEnvironment::new();
        databases.create_network();
        databases.start_cockroach();
        databases.start_postgres();
        databases.wait_for_cockroach();
        databases.wait_for_postgres();
        for mapping in MAPPINGS {
            databases.prepare_source_schema_and_seed(mapping.source_schema_sql);
            databases.prepare_destination_database(
                mapping.destination_database,
                mapping.destination_user,
                mapping.destination_password,
                mapping.destination_schema_sql,
            );
        }

        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let runner_port = pick_unused_port();
        let wrapper_bin_dir = temp_dir.path().join("bin");
        fs::create_dir_all(&wrapper_bin_dir).expect("wrapper bin dir should be created");

        let mut harness = Self {
            _database_test_guard: database_test_guard,
            databases,
            temp_dir,
            runner_port,
            runner_config_path: PathBuf::new(),
            wrapper_bin_dir,
            cockroach_wrapper_log_path: PathBuf::new(),
            runner_stdout_path: PathBuf::new(),
            runner_stderr_path: PathBuf::new(),
            runner_process: RefCell::new(None),
        };
        harness.materialize();
        harness
    }

    pub fn bootstrap_migration(&self) {
        self.start_runner_process();
        wait_for_runner_health(
            &https_client(&investigation_ca_cert_path()),
            self.runner_port,
            || self.runner_logs(),
        );
        self.apply_initial_changefeeds();
    }

    pub fn wait_for_initial_scan(&self, verify_image: &VerifyImageHarness) {
        self.wait_for_selected_tables_to_match_via_image(
            verify_image,
            APP_A,
            "app-a initial selected tables should match through the verify image",
        )
        .assert_selected_tables_match();
        self.wait_for_destination_query(
            APP_A.destination_database,
            APP_A_HELPER_SNAPSHOT_SQL,
            APP_A_INITIAL_SNAPSHOT,
            "app-a initial helper snapshot",
        );
        self.wait_for_selected_tables_to_match_via_image(
            verify_image,
            APP_B,
            "app-b initial selected tables should match through the verify image",
        )
        .assert_selected_tables_match();
        self.wait_for_destination_query(
            APP_B.destination_database,
            APP_B_HELPER_SNAPSHOT_SQL,
            APP_B_INITIAL_SNAPSHOT,
            "app-b initial helper snapshot",
        );
    }

    pub fn assert_helper_state_is_mapping_scoped(&self) {
        self.wait_for_destination_query(
            APP_A.destination_database,
            HELPER_TABLES_SQL,
            APP_A_HELPER_TABLES,
            "app-a helper table inventory",
        );
        self.wait_for_destination_query(
            APP_A.destination_database,
            STREAM_STATE_SQL,
            APP_A_STREAM_STATE,
            "app-a stream state",
        );
        self.wait_for_destination_query(
            APP_A.destination_database,
            TABLE_SYNC_STATE_SQL,
            APP_A_TABLE_SYNC_STATE,
            "app-a table sync state",
        );
        self.wait_for_destination_query(
            APP_B.destination_database,
            HELPER_TABLES_SQL,
            APP_B_HELPER_TABLES,
            "app-b helper table inventory",
        );
        self.wait_for_destination_query(
            APP_B.destination_database,
            STREAM_STATE_SQL,
            APP_B_STREAM_STATE,
            "app-b stream state",
        );
        self.wait_for_destination_query(
            APP_B.destination_database,
            TABLE_SYNC_STATE_SQL,
            APP_B_TABLE_SYNC_STATE,
            "app-b table sync state",
        );
    }

    pub fn apply_live_source_changes(&self) {
        run_audited_cockroach_sql(
            &self.wrapper_bin_dir,
            &format!(
                "USE {};\n{}",
                APP_A.source_database,
                r#"
UPDATE public.customers
SET email = 'alice+vip@example.com'
WHERE id = 1;
DELETE FROM public.customers
WHERE id = 2;
INSERT INTO public.customers (id, email)
VALUES (3, 'carol@example.com');

UPDATE public.order_items
SET sku = 'starter-kit-v2', quantity = 5
WHERE order_id = 100 AND line_id = 1;
DELETE FROM public.order_items
WHERE order_id = 100 AND line_id = 2;
INSERT INTO public.order_items (order_id, line_id, sku, quantity)
VALUES (101, 1, 'replacement-kit', 3);
"#,
            ),
        );
        run_audited_cockroach_sql(
            &self.wrapper_bin_dir,
            &format!(
                "USE {};\n{}",
                APP_B.source_database,
                r#"
UPDATE public.invoices
SET status = 'sent', amount_cents = 12500
WHERE id = 5001;
DELETE FROM public.invoice_lines
WHERE invoice_id = 5001 AND line_id = 2;
DELETE FROM public.invoice_lines
WHERE invoice_id = 5002 AND line_id = 1;
DELETE FROM public.invoices
WHERE id = 5002;
INSERT INTO public.invoices (id, status, amount_cents)
VALUES (5003, 'draft', 3300);
UPDATE public.invoice_lines
SET quantity = 4
WHERE invoice_id = 5001 AND line_id = 1;
INSERT INTO public.invoice_lines (invoice_id, line_id, sku, quantity)
VALUES (5003, 1, 'expansion-pack', 2);
"#,
            ),
        );
    }

    pub fn wait_for_live_catchup(&self, verify_image: &VerifyImageHarness) {
        self.wait_for_selected_tables_to_match_via_image(
            verify_image,
            APP_A,
            "app-a live selected tables should match through the verify image",
        )
        .assert_selected_tables_match();
        self.wait_for_destination_query(
            APP_A.destination_database,
            APP_A_HELPER_SNAPSHOT_SQL,
            APP_A_LIVE_SNAPSHOT,
            "app-a live helper snapshot",
        );
        self.wait_for_selected_tables_to_match_via_image(
            verify_image,
            APP_B,
            "app-b live selected tables should match through the verify image",
        )
        .assert_selected_tables_match();
        self.wait_for_destination_query(
            APP_B.destination_database,
            APP_B_HELPER_SNAPSHOT_SQL,
            APP_B_LIVE_SNAPSHOT,
            "app-b live helper snapshot",
        );
    }

    pub fn assert_mapping_state_stable(
        &self,
        verify_image: &VerifyImageHarness,
        duration: Duration,
    ) {
        self.assert_selected_tables_match_via_image_stable(
            verify_image,
            APP_A,
            "app-a selected tables should stay matched through the verify image",
            duration,
        );
        self.assert_destination_query_stable(
            APP_A.destination_database,
            APP_A_HELPER_SNAPSHOT_SQL,
            APP_A_LIVE_SNAPSHOT,
            "app-a helper snapshot after repeated reconcile",
            duration,
        );
        self.assert_destination_query_stable(
            APP_A.destination_database,
            HELPER_TABLES_SQL,
            APP_A_HELPER_TABLES,
            "app-a helper inventory after repeated reconcile",
            duration,
        );
        self.assert_selected_tables_match_via_image_stable(
            verify_image,
            APP_B,
            "app-b selected tables should stay matched through the verify image",
            duration,
        );
        self.assert_destination_query_stable(
            APP_B.destination_database,
            APP_B_HELPER_SNAPSHOT_SQL,
            APP_B_LIVE_SNAPSHOT,
            "app-b helper snapshot after repeated reconcile",
            duration,
        );
        self.assert_destination_query_stable(
            APP_B.destination_database,
            HELPER_TABLES_SQL,
            APP_B_HELPER_TABLES,
            "app-b helper inventory after repeated reconcile",
            duration,
        );
    }

    fn verify_selected_tables_via_image(
        &self,
        verify_image: &VerifyImageHarness,
        mapping: MappingSpec,
    ) -> VerifyCorrectnessAudit {
        let (include_schema_pattern, include_table_pattern) =
            verify_filter_patterns(mapping.selected_tables);
        verify_image.run_correctness_audit(&VerifyImageRun {
            source_url: self.databases.verify_source_url(mapping.source_database),
            source_ca_cert_path: self.databases.cockroach_ca_cert_path(),
            source_client_cert_path: self.databases.cockroach_client_cert_path(),
            source_client_key_path: self.databases.cockroach_client_key_path(),
            destination_url: self.databases.verify_destination_url(
                mapping.destination_database,
                mapping.destination_user,
                mapping.destination_password,
            ),
            destination_ca_cert_path: self.databases.postgres_ca_cert_path(),
            include_schema_pattern,
            include_table_pattern,
            expected_tables: mapping
                .selected_tables
                .iter()
                .map(|table| (*table).to_owned())
                .collect(),
        })
    }

    fn wait_for_selected_tables_to_match_via_image(
        &self,
        verify_image: &VerifyImageHarness,
        mapping: MappingSpec,
        description: &str,
    ) -> VerifyCorrectnessAudit {
        for _ in 0..60 {
            self.assert_runner_alive();
            let audit = self.verify_selected_tables_via_image(verify_image, mapping);
            if audit.selected_tables_match() {
                return audit;
            }
            thread::sleep(Duration::from_secs(1));
        }

        let audit = self.verify_selected_tables_via_image(verify_image, mapping);
        panic!(
            "{description} did not converge through the verify image correctness boundary\nmapping={}\nfinal audit={audit:?}\nrunner stderr:\n{}",
            mapping.id,
            read_file(&self.runner_stderr_path),
        );
    }

    fn assert_selected_tables_match_via_image_stable(
        &self,
        verify_image: &VerifyImageHarness,
        mapping: MappingSpec,
        description: &str,
        duration: Duration,
    ) {
        let deadline = Instant::now() + duration;
        loop {
            self.assert_runner_alive();
            let audit = self.verify_selected_tables_via_image(verify_image, mapping);
            assert!(
                audit.selected_tables_match(),
                "{description} stopped matching through the verify image correctness boundary\nmapping={}\nfinal audit={audit:?}\nrunner stderr:\n{}",
                mapping.id,
                read_file(&self.runner_stderr_path),
            );
            if Instant::now() >= deadline {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }
    }

    fn materialize(&mut self) {
        self.runner_config_path = self.temp_dir.path().join("runner.yml");
        self.cockroach_wrapper_log_path = self.temp_dir.path().join("cockroach-wrapper.log");
        self.runner_stdout_path = self.temp_dir.path().join("runner.stdout.log");
        self.runner_stderr_path = self.temp_dir.path().join("runner.stderr.log");

        write_cockroach_wrapper_script(
            &self.wrapper_bin_dir.join("cockroach"),
            &self.cockroach_wrapper_log_path,
            self.databases.cockroach_certs_dir(),
            self.databases.cockroach_sql_port(),
        );
        self.write_runner_config();
    }

    fn start_runner_process(&self) {
        if self.runner_process.borrow().is_some() {
            return;
        }

        let stdout = File::create(&self.runner_stdout_path).expect("runner stdout log should open");
        let stderr = File::create(&self.runner_stderr_path).expect("runner stderr log should open");
        let child = Command::new(env!("CARGO_BIN_EXE_runner"))
            .args(["run", "--config"])
            .arg(&self.runner_config_path)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .expect("runner process should start");
        *self.runner_process.borrow_mut() = Some(child);
    }

    fn write_runner_config(&self) {
        let mappings_yaml = MAPPINGS
            .iter()
            .map(|mapping| {
                let selected_tables = mapping
                    .selected_tables
                    .iter()
                    .map(|table| format!("        - {table}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    r#"  - id: {mapping_id}
    source:
      database: {source_database}
      tables:
{selected_tables}
    destination:
      host: 127.0.0.1
      port: {postgres_port}
      database: {destination_database}
      user: {destination_user}
      password: {destination_password}
"#,
                    mapping_id = mapping.id,
                    source_database = mapping.source_database,
                    selected_tables = selected_tables,
                    postgres_port = self.databases.postgres_host_port,
                    destination_database = mapping.destination_database,
                    destination_user = mapping.destination_user,
                    destination_password = mapping.destination_password,
                )
            })
            .collect::<Vec<_>>()
            .join("");

        fs::write(
            &self.runner_config_path,
            format!(
                r#"webhook:
  bind_addr: 0.0.0.0:{runner_port}
  tls:
    cert_path: {cert_path}
    key_path: {key_path}
reconcile:
  interval_secs: 1
mappings:
{mappings_yaml}"#,
                runner_port = self.runner_port,
                cert_path = investigation_server_cert_path().display(),
                key_path = investigation_server_key_path().display(),
                mappings_yaml = mappings_yaml,
            ),
        )
        .expect("runner config should be written");
    }

    fn apply_initial_changefeeds(&self) {
        run_audited_cockroach_sql(
            &self.wrapper_bin_dir,
            "SET CLUSTER SETTING kv.rangefeed.enabled = true;",
        );
        for mapping in MAPPINGS {
            let cursor = self.current_changefeed_cursor(mapping.source_database);
            let sql = self.render_changefeed_sql(mapping, &cursor);
            run_audited_cockroach_sql(&self.wrapper_bin_dir, &sql);
        }
    }

    fn current_changefeed_cursor(&self, database: &str) -> String {
        let output = run_audited_cockroach_sql(
            &self.wrapper_bin_dir,
            &format!("USE {database};\nSELECT cluster_logical_timestamp() AS changefeed_cursor;"),
        );
        output
            .lines()
            .map(str::trim)
            .rfind(|line| !line.is_empty() && *line != "changefeed_cursor")
            .unwrap_or_else(|| {
                panic!(
                    "cluster_logical_timestamp output should include a cursor row, got:\n{output}"
                )
            })
            .to_owned()
    }

    fn render_changefeed_sql(&self, mapping: MappingSpec, cursor: &str) -> String {
        let table_list = mapping
            .selected_tables
            .iter()
            .map(|table| format!("{}.{}", mapping.source_database, table))
            .collect::<Vec<_>>()
            .join(", ");
        let sink_url = self.changefeed_sink_url(mapping.id);
        format!(
            "CREATE CHANGEFEED FOR TABLE {table_list} INTO '{sink_url}' WITH cursor = '{cursor}', initial_scan = 'yes', envelope = 'enriched', enriched_properties = 'source', resolved = '{CHANGEFEED_RESOLVED_INTERVAL}';"
        )
    }

    fn changefeed_sink_url(&self, mapping_id: &str) -> String {
        let ca_cert_bytes = fs::read(investigation_ca_cert_path())
            .expect("changefeed CA certificate should be readable");
        let ca_cert_query = encode_ca_cert_query_value(&ca_cert_bytes);
        format!(
            "webhook-https://127.0.0.1:{}{}?ca_cert={}",
            self.runner_port,
            MappingIngestPath::new(mapping_id),
            ca_cert_query,
        )
    }

    fn wait_for_destination_query(
        &self,
        database: &str,
        sql: &str,
        expected: &str,
        description: &str,
    ) {
        for _ in 0..120 {
            self.assert_runner_alive();
            let actual = self.query_destination(database, sql);
            if actual.trim() == expected {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        panic!(
            "{description} did not converge to `{expected}`\ndatabase={database}\nactual={}\nrunner stderr:\n{}",
            self.query_destination(database, sql).trim(),
            read_file(&self.runner_stderr_path),
        );
    }

    fn assert_destination_query_stable(
        &self,
        database: &str,
        sql: &str,
        expected: &str,
        description: &str,
        duration: Duration,
    ) {
        let deadline = Instant::now() + duration;
        loop {
            self.assert_runner_alive();
            let actual = self.query_destination(database, sql);
            assert_eq!(
                actual.trim(),
                expected,
                "{description} changed unexpectedly while it should remain stable\ndatabase={database}\nrunner stderr:\n{}",
                read_file(&self.runner_stderr_path),
            );
            if Instant::now() >= deadline {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }
    }

    fn query_destination(&self, database: &str, sql: &str) -> String {
        self.databases.exec_psql(database, sql)
    }

    fn assert_runner_alive(&self) {
        let mut process = self.runner_process.borrow_mut();
        let Some(child) = process.as_mut() else {
            return;
        };

        if let Some(status) = child
            .try_wait()
            .expect("runner process status should be readable")
        {
            panic!(
                "runner exited early with status {status}\nstdout:\n{}\nstderr:\n{}",
                read_file(&self.runner_stdout_path),
                read_file(&self.runner_stderr_path),
            );
        }
    }

    fn runner_logs(&self) -> String {
        format!(
            "stdout:\n{}\n\nstderr:\n{}",
            read_file(&self.runner_stdout_path),
            read_file(&self.runner_stderr_path),
        )
    }
}

impl Drop for MultiMappingHarness {
    fn drop(&mut self) {
        if let Some(child) = self.runner_process.borrow_mut().as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn verify_filter_patterns(selected_tables: &[&str]) -> (String, String) {
    let mut schemas = selected_tables
        .iter()
        .map(|table| split_table_reference(table).0.to_owned())
        .collect::<Vec<_>>();
    schemas.sort();
    schemas.dedup();

    let mut tables = selected_tables
        .iter()
        .map(|table| split_table_reference(table).1.to_owned())
        .collect::<Vec<_>>();
    tables.sort();
    tables.dedup();

    (
        anchored_posix_union(&schemas),
        anchored_posix_union(&tables),
    )
}

fn split_table_reference(table: &str) -> (&str, &str) {
    table
        .split_once('.')
        .unwrap_or_else(|| panic!("mapped table should be qualified as schema.table: {table}"))
}

fn anchored_posix_union(values: &[String]) -> String {
    format!("^({})$", values.join("|"))
}
