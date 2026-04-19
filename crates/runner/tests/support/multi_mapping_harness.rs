use std::{
    cell::RefCell,
    fs::{self, File},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::MutexGuard,
    thread,
    time::{Duration, Instant},
};

use clap::Parser as _;
use tempfile::TempDir;

use crate::e2e_harness::{
    DockerEnvironment, https_client, investigation_ca_cert_path, investigation_server_cert_path,
    investigation_server_key_path, lock_e2e_docker_resources, pick_unused_port, read_file,
    run_audited_cockroach_sql, source_bootstrap_sql_statements, wait_for_runner_health,
    write_cockroach_wrapper_script,
};
use crate::e2e_integrity::SourceCommandAudit;

const APP_A_SOURCE_SETUP_SQL: &str = r#"
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

const APP_A_DESTINATION_SETUP_SQL: &str = r#"
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

const APP_B_SOURCE_SETUP_SQL: &str = r#"
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

const APP_B_DESTINATION_SETUP_SQL: &str = r#"
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

const APP_A_REAL_SNAPSHOT_SQL: &str = r#"
SELECT string_agg(entry, ',' ORDER BY entry)
FROM (
    SELECT 'c:' || id::text || ':' || email AS entry FROM public.customers
    UNION ALL
    SELECT 'o:' || order_id::text || '|' || line_id::text || '|' || sku || '|' || quantity::text
    FROM public.order_items
) snapshot;
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

const APP_B_REAL_SNAPSHOT_SQL: &str = r#"
SELECT string_agg(entry, ',' ORDER BY entry)
FROM (
    SELECT 'i:' || id::text || ':' || status || ':' || amount_cents::text AS entry
    FROM public.invoices
    UNION ALL
    SELECT 'l:' || invoice_id::text || '|' || line_id::text || '|' || sku || '|' || quantity::text
    FROM public.invoice_lines
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
    source_setup_sql: &'static str,
    destination_setup_sql: &'static str,
}

const APP_A: MappingSpec = MappingSpec {
    id: "app-a",
    source_database: "demo_a",
    destination_database: "app_a",
    destination_user: "migration_user_a",
    destination_password: "runner-secret-a",
    selected_tables: &["public.customers", "public.order_items"],
    source_setup_sql: APP_A_SOURCE_SETUP_SQL,
    destination_setup_sql: APP_A_DESTINATION_SETUP_SQL,
};

const APP_B: MappingSpec = MappingSpec {
    id: "app-b",
    source_database: "demo_b",
    destination_database: "app_b",
    destination_user: "migration_user_b",
    destination_password: "runner-secret-b",
    selected_tables: &["public.invoices", "public.invoice_lines"],
    source_setup_sql: APP_B_SOURCE_SETUP_SQL,
    destination_setup_sql: APP_B_DESTINATION_SETUP_SQL,
};

const MAPPINGS: [MappingSpec; 2] = [APP_A, APP_B];

pub struct MultiMappingHarness {
    _docker_test_guard: MutexGuard<'static, ()>,
    docker: DockerEnvironment,
    temp_dir: TempDir,
    runner_port: u16,
    runner_config_path: PathBuf,
    source_bootstrap_config_path: PathBuf,
    wrapper_bin_dir: PathBuf,
    cockroach_wrapper_log_path: PathBuf,
    runner_stdout_path: PathBuf,
    runner_stderr_path: PathBuf,
    runner_process: RefCell<Option<Child>>,
}

impl MultiMappingHarness {
    pub fn start() -> Self {
        let docker_test_guard = lock_e2e_docker_resources();
        let docker = DockerEnvironment::new();
        docker.create_network();
        docker.start_cockroach();
        docker.start_postgres();
        docker.wait_for_cockroach();
        docker.wait_for_postgres();
        for mapping in MAPPINGS {
            docker.prepare_source_schema_and_seed(mapping.source_setup_sql);
            docker.prepare_destination_database(
                mapping.destination_database,
                mapping.destination_user,
                mapping.destination_password,
                mapping.destination_setup_sql,
            );
        }

        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let runner_port = pick_unused_port();
        let wrapper_bin_dir = temp_dir.path().join("bin");
        fs::create_dir_all(&wrapper_bin_dir).expect("wrapper bin dir should be created");

        let mut harness = Self {
            _docker_test_guard: docker_test_guard,
            docker,
            temp_dir,
            runner_port,
            runner_config_path: PathBuf::new(),
            source_bootstrap_config_path: PathBuf::new(),
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
        let source_bootstrap_sql = self.render_source_bootstrap_sql();
        self.apply_source_bootstrap_sql(&source_bootstrap_sql);
    }

    pub fn wait_for_initial_scan(&self) {
        self.wait_for_destination_query(
            APP_A.destination_database,
            APP_A_REAL_SNAPSHOT_SQL,
            APP_A_INITIAL_SNAPSHOT,
            "app-a initial real snapshot",
        );
        self.wait_for_destination_query(
            APP_A.destination_database,
            APP_A_HELPER_SNAPSHOT_SQL,
            APP_A_INITIAL_SNAPSHOT,
            "app-a initial helper snapshot",
        );
        self.wait_for_destination_query(
            APP_B.destination_database,
            APP_B_REAL_SNAPSHOT_SQL,
            APP_B_INITIAL_SNAPSHOT,
            "app-b initial real snapshot",
        );
        self.wait_for_destination_query(
            APP_B.destination_database,
            APP_B_HELPER_SNAPSHOT_SQL,
            APP_B_INITIAL_SNAPSHOT,
            "app-b initial helper snapshot",
        );
    }

    pub fn assert_explicit_source_bootstrap_commands(&self) {
        let audit = self.source_command_audit();
        audit.assert_bootstrap_command_count(4);
        audit.assert_bootstrap_contains(
            "SET CLUSTER SETTING kv.rangefeed.enabled = true;",
            "bootstrap should issue one rangefeed enable, one start cursor, and one changefeed per mapping",
        );
        audit.assert_bootstrap_contains(
            "SELECT cluster_logical_timestamp();",
            "bootstrap should capture the start cursor explicitly",
        );
        audit.assert_bootstrap_contains(
            "CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.order_items",
            "bootstrap should create the app-a changefeed explicitly",
        );
        audit.assert_bootstrap_contains(
            "CREATE CHANGEFEED FOR TABLE demo_b.public.invoices, demo_b.public.invoice_lines",
            "bootstrap should create the app-b changefeed explicitly",
        );
    }

    fn source_command_audit(&self) -> SourceCommandAudit {
        SourceCommandAudit::from_cockroach_log(&self.cockroach_wrapper_log_path, 4)
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

    pub fn wait_for_live_catchup(&self) {
        self.wait_for_destination_query(
            APP_A.destination_database,
            APP_A_REAL_SNAPSHOT_SQL,
            APP_A_LIVE_SNAPSHOT,
            "app-a live real snapshot",
        );
        self.wait_for_destination_query(
            APP_A.destination_database,
            APP_A_HELPER_SNAPSHOT_SQL,
            APP_A_LIVE_SNAPSHOT,
            "app-a live helper snapshot",
        );
        self.wait_for_destination_query(
            APP_B.destination_database,
            APP_B_REAL_SNAPSHOT_SQL,
            APP_B_LIVE_SNAPSHOT,
            "app-b live real snapshot",
        );
        self.wait_for_destination_query(
            APP_B.destination_database,
            APP_B_HELPER_SNAPSHOT_SQL,
            APP_B_LIVE_SNAPSHOT,
            "app-b live helper snapshot",
        );
    }

    pub fn assert_mapping_state_stable(&self, duration: Duration) {
        self.assert_destination_query_stable(
            APP_A.destination_database,
            APP_A_REAL_SNAPSHOT_SQL,
            APP_A_LIVE_SNAPSHOT,
            "app-a destination snapshot after repeated reconcile",
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
        self.assert_destination_query_stable(
            APP_B.destination_database,
            APP_B_REAL_SNAPSHOT_SQL,
            APP_B_LIVE_SNAPSHOT,
            "app-b destination snapshot after repeated reconcile",
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

    fn materialize(&mut self) {
        self.runner_config_path = self.temp_dir.path().join("runner.yml");
        self.source_bootstrap_config_path = self.temp_dir.path().join("source-bootstrap.yml");
        self.cockroach_wrapper_log_path = self.temp_dir.path().join("cockroach-wrapper.log");
        self.runner_stdout_path = self.temp_dir.path().join("runner.stdout.log");
        self.runner_stderr_path = self.temp_dir.path().join("runner.stderr.log");

        write_cockroach_wrapper_script(
            &self.wrapper_bin_dir.join("cockroach"),
            &self.cockroach_wrapper_log_path,
            &self.docker.cockroach_container,
        );
        self.write_runner_config();
        self.write_source_bootstrap_config();
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

    fn render_source_bootstrap_sql(&self) -> String {
        source_bootstrap::execute(source_bootstrap::Cli::parse_from([
            "source-bootstrap",
            "render-bootstrap-sql",
            "--config",
            self.source_bootstrap_config_path
                .to_str()
                .expect("source-bootstrap config path should be utf-8"),
        ]))
        .unwrap_or_else(|error| panic!("source-bootstrap render-bootstrap-sql failed: {error}"))
    }

    fn apply_source_bootstrap_sql(&self, sql: &str) {
        for statement in source_bootstrap_sql_statements(sql) {
            run_audited_cockroach_sql(&self.wrapper_bin_dir, &statement);
        }
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
      connection:
        host: 127.0.0.1
        port: {postgres_port}
        database: {destination_database}
        user: {destination_user}
        password: {destination_password}
"#,
                    mapping_id = mapping.id,
                    source_database = mapping.source_database,
                    selected_tables = selected_tables,
                    postgres_port = self.docker.postgres_host_port,
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

    fn write_source_bootstrap_config(&self) {
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
"#,
                    mapping_id = mapping.id,
                    source_database = mapping.source_database,
                    selected_tables = selected_tables,
                )
            })
            .collect::<Vec<_>>()
            .join("");

        fs::write(
            &self.source_bootstrap_config_path,
            format!(
                r#"cockroach:
  url: postgresql://root@127.0.0.1:26257/defaultdb?sslmode=disable
webhook:
  base_url: https://host.docker.internal:{runner_port}
  ca_cert_path: {ca_cert_path}
  resolved: 1s
mappings:
{mappings_yaml}"#,
                runner_port = self.runner_port,
                ca_cert_path = investigation_ca_cert_path().display(),
                mappings_yaml = mappings_yaml,
            ),
        )
        .expect("source-bootstrap config should be written");
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
        self.docker.exec_psql(database, sql)
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
