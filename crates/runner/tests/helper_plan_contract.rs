use std::fs;

use assert_cmd::Command;
use predicates::prelude::predicate;

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

fn render_helper_plan_command(config_path: &std::path::Path) -> Command {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");
    command.args([
        "render-helper-plan",
        "--config",
        config_path.to_str().expect("config path should be utf-8"),
        "--mapping",
        "app-a",
    ]);
    command
}

#[test]
fn render_helper_plan_writes_helper_table_sql_for_a_selected_mapping() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    let cockroach_schema_path = temp_dir.path().join("cockroach.sql");
    let postgres_schema_path = temp_dir.path().join("postgres.sql");
    let output_dir = temp_dir.path().join("artifacts");

    write_runner_config(&config_path, &["public.customers"]);
    fs::write(
        &cockroach_schema_path,
        r#"SET
create_statement
"CREATE TABLE public.customers (
    id INT8 NOT NULL,
    email STRING NOT NULL,
    nickname STRING NULL,
    CONSTRAINT customers_pkey PRIMARY KEY (id ASC)
) WITH (schema_locked = true);"
"#,
    )
    .expect("cockroach schema should be written");
    fs::write(
        &postgres_schema_path,
        r#"CREATE TABLE public.customers (
    id bigint NOT NULL,
    email text NOT NULL,
    nickname text
);

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_pkey PRIMARY KEY (id);
"#,
    )
    .expect("postgres schema should be written");

    render_helper_plan_command(&config_path)
        .args(["--cockroach-schema"])
        .arg(&cockroach_schema_path)
        .args(["--postgres-schema"])
        .arg(&postgres_schema_path)
        .args(["--output-dir"])
        .arg(&output_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("mapping=app-a"))
        .stdout(predicate::str::contains("helper_tables=1"));

    let helper_sql = fs::read_to_string(output_dir.join("app-a").join("helper_tables.sql"))
        .expect("helper table sql should be written");
    assert_eq!(
        helper_sql,
        concat!(
            "CREATE TABLE IF NOT EXISTS \"_cockroach_migration_tool\".\"app-a__public__customers\" (\n",
            "    \"id\" bigint NOT NULL,\n",
            "    \"email\" text NOT NULL,\n",
            "    \"nickname\" text\n",
            ");\n",
        )
    );
}

#[test]
fn render_helper_plan_strips_serving_structure_from_helper_table_ddl() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    let cockroach_schema_path = temp_dir.path().join("cockroach.sql");
    let postgres_schema_path = temp_dir.path().join("postgres.sql");
    let output_dir = temp_dir.path().join("artifacts");

    write_runner_config(&config_path, &["public.orders"]);
    fs::write(
        &cockroach_schema_path,
        r#"SET
create_statement
"CREATE TABLE public.orders (
    id INT8 NOT NULL,
    customer_id INT8 NULL,
    status STRING NOT NULL,
    total_cents INT8 NULL,
    CONSTRAINT orders_pkey PRIMARY KEY (id ASC)
) WITH (schema_locked = true);"
ALTER TABLE public.orders ADD CONSTRAINT orders_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES public.customers(id);
CREATE UNIQUE INDEX orders_status_unique ON public.orders (status);
CREATE INDEX orders_customer_lookup ON public.orders (customer_id);
"#,
    )
    .expect("cockroach schema should be written");
    fs::write(
        &postgres_schema_path,
        r#"CREATE TABLE public.orders (
    id bigint NOT NULL,
    customer_id bigint,
    status text NOT NULL DEFAULT 'pending',
    total_cents bigint GENERATED ALWAYS AS (customer_id * 100) STORED
);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES public.customers(id);

CREATE UNIQUE INDEX orders_status_unique
    ON public.orders USING btree (status);

CREATE INDEX orders_customer_lookup
    ON public.orders USING btree (customer_id);
"#,
    )
    .expect("postgres schema should be written");

    render_helper_plan_command(&config_path)
        .args(["--cockroach-schema"])
        .arg(&cockroach_schema_path)
        .args(["--postgres-schema"])
        .arg(&postgres_schema_path)
        .args(["--output-dir"])
        .arg(&output_dir)
        .assert()
        .success();

    let helper_sql = fs::read_to_string(output_dir.join("app-a").join("helper_tables.sql"))
        .expect("helper table sql should be written");
    assert_eq!(
        helper_sql,
        concat!(
            "CREATE TABLE IF NOT EXISTS \"_cockroach_migration_tool\".\"app-a__public__orders\" (\n",
            "    \"id\" bigint NOT NULL,\n",
            "    \"customer_id\" bigint,\n",
            "    \"status\" text NOT NULL,\n",
            "    \"total_cents\" bigint\n",
            ");\n",
        )
    );
    assert!(!helper_sql.contains("DEFAULT"));
    assert!(!helper_sql.contains("GENERATED ALWAYS"));
    assert!(!helper_sql.contains("PRIMARY KEY"));
    assert!(!helper_sql.contains("UNIQUE"));
    assert!(!helper_sql.contains("REFERENCES"));
    assert!(!helper_sql.contains("CREATE INDEX"));
}

#[test]
fn render_helper_plan_writes_parent_before_child_and_reverse_delete_order() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    let cockroach_schema_path = temp_dir.path().join("cockroach.sql");
    let postgres_schema_path = temp_dir.path().join("postgres.sql");
    let output_dir = temp_dir.path().join("artifacts");

    write_runner_config(
        &config_path,
        &[
            "public.order_items",
            "public.orders",
            "public.customers",
        ],
    );
    fs::write(
        &cockroach_schema_path,
        r#"SET
create_statement
"CREATE TABLE public.customers (
    id INT8 NOT NULL,
    CONSTRAINT customers_pkey PRIMARY KEY (id ASC)
) WITH (schema_locked = true);"
"CREATE TABLE public.orders (
    id INT8 NOT NULL,
    customer_id INT8 NOT NULL,
    CONSTRAINT orders_pkey PRIMARY KEY (id ASC),
    INDEX orders_customer_lookup (customer_id ASC)
) WITH (schema_locked = true);"
"CREATE TABLE public.order_items (
    id INT8 NOT NULL,
    order_id INT8 NOT NULL,
    CONSTRAINT order_items_pkey PRIMARY KEY (id ASC),
    INDEX order_items_order_lookup (order_id ASC)
) WITH (schema_locked = true);"
ALTER TABLE public.orders ADD CONSTRAINT orders_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES public.customers(id);
ALTER TABLE public.order_items ADD CONSTRAINT order_items_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);
"#,
    )
    .expect("cockroach schema should be written");
    fs::write(
        &postgres_schema_path,
        r#"CREATE TABLE public.customers (
    id bigint NOT NULL
);

CREATE TABLE public.orders (
    id bigint NOT NULL,
    customer_id bigint NOT NULL
);

CREATE TABLE public.order_items (
    id bigint NOT NULL,
    order_id bigint NOT NULL
);

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.order_items
    ADD CONSTRAINT order_items_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES public.customers(id);

ALTER TABLE ONLY public.order_items
    ADD CONSTRAINT order_items_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);

CREATE INDEX orders_customer_lookup
    ON public.orders USING btree (customer_id);

CREATE INDEX order_items_order_lookup
    ON public.order_items USING btree (order_id);
"#,
    )
    .expect("postgres schema should be written");

    render_helper_plan_command(&config_path)
        .args(["--cockroach-schema"])
        .arg(&cockroach_schema_path)
        .args(["--postgres-schema"])
        .arg(&postgres_schema_path)
        .args(["--output-dir"])
        .arg(&output_dir)
        .assert()
        .success();

    let reconcile_order = fs::read_to_string(output_dir.join("app-a").join("reconcile_order.txt"))
        .expect("reconcile order should be written");
    assert_eq!(
        reconcile_order,
        concat!(
            "upsert:\n",
            "public.customers\n",
            "public.orders\n",
            "public.order_items\n",
            "delete:\n",
            "public.order_items\n",
            "public.orders\n",
            "public.customers\n",
        )
    );
}

#[test]
fn render_helper_plan_fails_loudly_for_dependency_cycles() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    let cockroach_schema_path = temp_dir.path().join("cockroach.sql");
    let postgres_schema_path = temp_dir.path().join("postgres.sql");
    let output_dir = temp_dir.path().join("artifacts");

    write_runner_config(&config_path, &["public.accounts", "public.account_links"]);
    fs::write(
        &cockroach_schema_path,
        r#"SET
create_statement
"CREATE TABLE public.accounts (
    id INT8 NOT NULL,
    link_id INT8 NULL,
    CONSTRAINT accounts_pkey PRIMARY KEY (id ASC),
    INDEX accounts_link_lookup (link_id ASC)
) WITH (schema_locked = true);"
"CREATE TABLE public.account_links (
    id INT8 NOT NULL,
    account_id INT8 NOT NULL,
    CONSTRAINT account_links_pkey PRIMARY KEY (id ASC),
    INDEX account_links_account_lookup (account_id ASC)
) WITH (schema_locked = true);"
ALTER TABLE public.accounts ADD CONSTRAINT accounts_link_id_fkey FOREIGN KEY (link_id) REFERENCES public.account_links(id);
ALTER TABLE public.account_links ADD CONSTRAINT account_links_account_id_fkey FOREIGN KEY (account_id) REFERENCES public.accounts(id);
"#,
    )
    .expect("cockroach schema should be written");
    fs::write(
        &postgres_schema_path,
        r#"CREATE TABLE public.accounts (
    id bigint NOT NULL,
    link_id bigint
);

CREATE TABLE public.account_links (
    id bigint NOT NULL,
    account_id bigint NOT NULL
);

ALTER TABLE ONLY public.accounts
    ADD CONSTRAINT accounts_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.account_links
    ADD CONSTRAINT account_links_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.accounts
    ADD CONSTRAINT accounts_link_id_fkey FOREIGN KEY (link_id) REFERENCES public.account_links(id);

ALTER TABLE ONLY public.account_links
    ADD CONSTRAINT account_links_account_id_fkey FOREIGN KEY (account_id) REFERENCES public.accounts(id);

CREATE INDEX accounts_link_lookup
    ON public.accounts USING btree (link_id);

CREATE INDEX account_links_account_lookup
    ON public.account_links USING btree (account_id);
"#,
    )
    .expect("postgres schema should be written");

    render_helper_plan_command(&config_path)
        .args(["--cockroach-schema"])
        .arg(&cockroach_schema_path)
        .args(["--postgres-schema"])
        .arg(&postgres_schema_path)
        .args(["--output-dir"])
        .arg(&output_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "helper plan: dependency cycle detected for mapping `app-a`",
        ))
        .stderr(predicate::str::contains("public.accounts"))
        .stderr(predicate::str::contains("public.account_links"));
}

#[test]
fn render_helper_plan_keeps_composite_pk_columns_out_of_base_helper_ddl() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    let cockroach_schema_path = temp_dir.path().join("cockroach.sql");
    let postgres_schema_path = temp_dir.path().join("postgres.sql");
    let output_dir = temp_dir.path().join("artifacts");

    write_runner_config(&config_path, &["public.orders"]);
    fs::write(
        &cockroach_schema_path,
        r#"SET
create_statement
"CREATE TABLE public.orders (
    tenant_id INT8 NOT NULL,
    order_id INT8 NOT NULL,
    total_cents INT8 NOT NULL,
    CONSTRAINT orders_pkey PRIMARY KEY (tenant_id ASC, order_id ASC)
) WITH (schema_locked = true);"
"#,
    )
    .expect("cockroach schema should be written");
    fs::write(
        &postgres_schema_path,
        r#"CREATE TABLE public.orders (
    tenant_id bigint NOT NULL,
    order_id bigint NOT NULL,
    total_cents bigint NOT NULL
);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_pkey PRIMARY KEY (tenant_id, order_id);
"#,
    )
    .expect("postgres schema should be written");

    render_helper_plan_command(&config_path)
        .args(["--cockroach-schema"])
        .arg(&cockroach_schema_path)
        .args(["--postgres-schema"])
        .arg(&postgres_schema_path)
        .args(["--output-dir"])
        .arg(&output_dir)
        .assert()
        .success();

    let helper_sql = fs::read_to_string(output_dir.join("app-a").join("helper_tables.sql"))
        .expect("helper table sql should be written");
    assert_eq!(
        helper_sql,
        concat!(
            "CREATE TABLE IF NOT EXISTS \"_cockroach_migration_tool\".\"app-a__public__orders\" (\n",
            "    \"tenant_id\" bigint NOT NULL,\n",
            "    \"order_id\" bigint NOT NULL,\n",
            "    \"total_cents\" bigint NOT NULL\n",
            ");\n",
        )
    );
    assert!(!helper_sql.contains("PRIMARY KEY"));
    assert!(!helper_sql.contains("CREATE INDEX"));
}
