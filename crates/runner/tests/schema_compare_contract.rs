use std::{fs, path::PathBuf};

use assert_cmd::Command;
use predicates::prelude::predicate;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("runner crate should have a workspace root")
        .parent()
        .expect("workspace root should exist")
        .to_path_buf()
}

fn investigated_schema_path(name: &str) -> PathBuf {
    repo_root()
        .join("investigations")
        .join("cockroach-webhook-cdc")
        .join("output")
        .join("schema-compare")
        .join(name)
}

fn write_runner_config(path: &std::path::Path, tables: &[&str]) {
    let tables_yaml = tables
        .iter()
        .map(|table| format!("        - {table}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        path,
        format!(
            r#"webhook:
  bind_addr: 127.0.0.1:8443
  tls:
    cert_path: certs/server.crt
    key_path: certs/server.key
reconcile:
  interval_secs: 30
verify:
  molt:
    command: molt
    report_dir: /tmp/molt
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
{tables_yaml}
    destination:
      connection:
        host: pg-a.example.internal
        port: 5432
        database: app_a
        user: migration_user_a
        password: runner-secret-a
"#,
        ),
    )
    .expect("config should be written");
}

fn compare_schema_command(config_path: &std::path::Path) -> Command {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");
    command.args([
        "compare-schema",
        "--config",
        config_path.to_str().expect("config path should be utf-8"),
        "--mapping",
        "app-a",
    ]);
    command
}

#[test]
fn compare_schema_accepts_semantically_matching_exports() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    write_runner_config(
        &config_path,
        &[
            "public.customers",
            "public.products",
            "public.orders",
            "public.order_items",
        ],
    );

    compare_schema_command(&config_path)
        .args(["--cockroach-schema"])
        .arg(investigated_schema_path("crdb_schema.txt"))
        .args(["--postgres-schema"])
        .arg(investigated_schema_path("pg_schema.sql"))
        .assert()
        .success()
        .stdout(predicate::str::contains("schema compatible"))
        .stdout(predicate::str::contains("mapping=app-a"))
        .stdout(predicate::str::contains("tables=4"))
        .stdout(predicate::str::contains("ignored_tables=0"));
}

#[test]
fn compare_schema_accepts_postgres_unique_indexes_as_unique_constraints() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    let cockroach_schema_path = temp_dir.path().join("cockroach.sql");
    let postgres_schema_path = temp_dir.path().join("postgres.sql");

    write_runner_config(&config_path, &["public.customers"]);
    fs::write(
        &cockroach_schema_path,
        r#"SET
create_statement
"CREATE TABLE public.customers (
    id INT8 NOT NULL,
    email STRING NOT NULL,
    CONSTRAINT customers_pkey PRIMARY KEY (id ASC),
    UNIQUE INDEX customers_email_key (email ASC)
) WITH (schema_locked = true);"
"#,
    )
    .expect("cockroach schema should be written");
    fs::write(
        &postgres_schema_path,
        r#"CREATE TABLE public.customers (
    id bigint NOT NULL,
    email character varying(255) NOT NULL
);

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_pkey PRIMARY KEY (id);

CREATE UNIQUE INDEX destination_email_unique
    ON public.customers USING btree (email);
"#,
    )
    .expect("postgres schema should be written");

    compare_schema_command(&config_path)
        .args(["--cockroach-schema"])
        .arg(&cockroach_schema_path)
        .args(["--postgres-schema"])
        .arg(&postgres_schema_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("schema compatible"))
        .stdout(predicate::str::contains("tables=1"));
}

#[test]
fn compare_schema_rejects_foreign_keys_with_different_on_delete_actions() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    let cockroach_schema_path = temp_dir.path().join("cockroach.sql");
    let postgres_schema_path = temp_dir.path().join("postgres.sql");

    write_runner_config(&config_path, &["public.orders"]);
    fs::write(
        &cockroach_schema_path,
        r#"SET
create_statement
"CREATE TABLE public.orders (
    id INT8 NOT NULL,
    customer_id INT8 NULL,
    CONSTRAINT orders_pkey PRIMARY KEY (id ASC)
) WITH (schema_locked = true);"
ALTER TABLE public.orders ADD CONSTRAINT orders_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES public.customers(id);
"#,
    )
    .expect("cockroach schema should be written");
    fs::write(
        &postgres_schema_path,
        r#"CREATE TABLE public.orders (
    id bigint NOT NULL,
    customer_id bigint
);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES public.customers(id) ON DELETE SET NULL;
"#,
    )
    .expect("postgres schema should be written");

    compare_schema_command(&config_path)
        .args(["--cockroach-schema"])
        .arg(&cockroach_schema_path)
        .args(["--postgres-schema"])
        .arg(&postgres_schema_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("foreign key mismatch"))
        .stderr(predicate::str::contains("on_delete=set_null"));
}

#[test]
fn compare_schema_ignores_tables_outside_the_selected_mapping() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    let cockroach_schema_path = temp_dir.path().join("cockroach.sql");
    let postgres_schema_path = temp_dir.path().join("postgres.sql");

    write_runner_config(&config_path, &["public.customers"]);
    fs::write(
        &cockroach_schema_path,
        r#"SET
create_statement
"CREATE TABLE public.customers (
    id INT8 NOT NULL,
    email STRING NOT NULL,
    CONSTRAINT customers_pkey PRIMARY KEY (id ASC)
) WITH (schema_locked = true);"
"CREATE TABLE public.orders (
    id INT8 NOT NULL,
    status STRING NOT NULL,
    CONSTRAINT orders_pkey PRIMARY KEY (id ASC)
) WITH (schema_locked = true);"
"#,
    )
    .expect("cockroach schema should be written");
    fs::write(
        &postgres_schema_path,
        r#"CREATE TABLE public.customers (
    id bigint NOT NULL,
    email character varying(255) NOT NULL
);

CREATE TABLE public.invoices (
    id bigint NOT NULL,
    total_cents bigint NOT NULL
);

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.invoices
    ADD CONSTRAINT invoices_pkey PRIMARY KEY (id);
"#,
    )
    .expect("postgres schema should be written");

    compare_schema_command(&config_path)
        .args(["--cockroach-schema"])
        .arg(&cockroach_schema_path)
        .args(["--postgres-schema"])
        .arg(&postgres_schema_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("schema compatible"))
        .stdout(predicate::str::contains("tables=1"))
        .stdout(predicate::str::contains("ignored_tables=2"));
}

#[test]
fn compare_schema_fails_loudly_for_unsupported_type_pairs() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    let cockroach_schema_path = temp_dir.path().join("cockroach.sql");
    let postgres_schema_path = temp_dir.path().join("postgres.sql");

    write_runner_config(&config_path, &["public.events"]);
    fs::write(
        &cockroach_schema_path,
        r#"SET
create_statement
"CREATE TABLE public.events (
    id INT8 NOT NULL,
    payload JSONB NOT NULL,
    CONSTRAINT events_pkey PRIMARY KEY (id ASC)
) WITH (schema_locked = true);"
"#,
    )
    .expect("cockroach schema should be written");
    fs::write(
        &postgres_schema_path,
        r#"CREATE TABLE public.events (
    id bigint NOT NULL,
    payload jsonb NOT NULL
);

ALTER TABLE ONLY public.events
    ADD CONSTRAINT events_pkey PRIMARY KEY (id);
"#,
    )
    .expect("postgres schema should be written");

    compare_schema_command(&config_path)
        .args(["--cockroach-schema"])
        .arg(&cockroach_schema_path)
        .args(["--postgres-schema"])
        .arg(&postgres_schema_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported type pair"))
        .stderr(predicate::str::contains("public.events.payload"));
}

#[test]
fn compare_schema_reports_missing_selected_columns() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    let cockroach_schema_path = temp_dir.path().join("cockroach.sql");
    let postgres_schema_path = temp_dir.path().join("postgres.sql");

    write_runner_config(&config_path, &["public.customers"]);
    fs::write(
        &cockroach_schema_path,
        r#"SET
create_statement
"CREATE TABLE public.customers (
    id INT8 NOT NULL,
    email STRING NOT NULL,
    region STRING NOT NULL,
    CONSTRAINT customers_pkey PRIMARY KEY (id ASC)
) WITH (schema_locked = true);"
"#,
    )
    .expect("cockroach schema should be written");
    fs::write(
        &postgres_schema_path,
        r#"CREATE TABLE public.customers (
    id bigint NOT NULL,
    email character varying(255) NOT NULL
);

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_pkey PRIMARY KEY (id);
"#,
    )
    .expect("postgres schema should be written");

    compare_schema_command(&config_path)
        .args(["--cockroach-schema"])
        .arg(&cockroach_schema_path)
        .args(["--postgres-schema"])
        .arg(&postgres_schema_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing column on postgres"))
        .stderr(predicate::str::contains("public.customers.region"));
}

#[test]
fn compare_schema_rejects_non_unique_index_shape_mismatches() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    let cockroach_schema_path = temp_dir.path().join("cockroach.sql");
    let postgres_schema_path = temp_dir.path().join("postgres.sql");

    write_runner_config(&config_path, &["public.orders"]);
    fs::write(
        &cockroach_schema_path,
        r#"SET
create_statement
"CREATE TABLE public.orders (
    id INT8 NOT NULL,
    customer_id INT8 NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT orders_pkey PRIMARY KEY (id ASC),
    INDEX orders_customer_created_idx (customer_id ASC, created_at DESC)
) WITH (schema_locked = true);"
"#,
    )
    .expect("cockroach schema should be written");
    fs::write(
        &postgres_schema_path,
        r#"CREATE TABLE public.orders (
    id bigint NOT NULL,
    customer_id bigint NOT NULL,
    created_at timestamp with time zone NOT NULL
);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_pkey PRIMARY KEY (id);

CREATE INDEX orders_customer_created_idx
    ON public.orders USING btree (customer_id, created_at);
"#,
    )
    .expect("postgres schema should be written");

    compare_schema_command(&config_path)
        .args(["--cockroach-schema"])
        .arg(&cockroach_schema_path)
        .args(["--postgres-schema"])
        .arg(&postgres_schema_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("index mismatch"))
        .stderr(predicate::str::contains("created_at DESC"));
}
