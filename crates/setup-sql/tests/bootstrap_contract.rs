use std::{fs, path::PathBuf};

#[path = "support/readme_contract.rs"]
mod readme_contract_support;

use assert_cmd::Command;
use predicates::prelude::{PredicateBooleanExt, predicate};
use readme_contract_support::RepositoryReadme;
use serde_json::Value;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

const FIXTURE_CA_CERT_QUERY: &str = "ZHVtbXktY2EK";

#[test]
fn emit_cockroach_sql_defaults_to_text_and_supports_simple_json_output() {
    let mut text_command = Command::cargo_bin("setup-sql").expect("setup-sql binary should exist");

    text_command
        .args(["emit-cockroach-sql", "--config"])
        .arg(fixture_path("valid-cockroach-setup-config.yml"))
        .assert()
        .success()
        .stdout(predicate::str::starts_with("-- Source bootstrap SQL\n"))
        .stdout(predicate::str::contains(
            "SET CLUSTER SETTING kv.rangefeed.enabled = true;",
        ))
        .stdout(predicate::str::contains(
            "SELECT cluster_logical_timestamp();",
        ))
        .stdout(predicate::str::contains("-- Mapping: app-a"))
        .stdout(predicate::str::contains("-- Source database: demo_a"))
        .stdout(predicate::str::contains(
            "-- Selected tables: public.customers, public.orders",
        ))
        .stdout(predicate::str::contains("-- Mapping: app-b"))
        .stdout(predicate::str::contains(
            "CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders",
        ))
        .stdout(predicate::str::contains(
            "CREATE CHANGEFEED FOR TABLE demo_b.public.invoices",
        ))
        .stdout(predicate::str::contains(format!(
            "INTO 'webhook-https://runner.example.internal:8443/ingest/app-a?ca_cert={FIXTURE_CA_CERT_QUERY}'"
        )))
        .stdout(predicate::str::contains(format!(
            "INTO 'webhook-https://runner.example.internal:8443/ingest/app-b?ca_cert={FIXTURE_CA_CERT_QUERY}'"
        )))
        .stdout(predicate::str::contains("initial_scan = 'yes'"))
        .stdout(predicate::str::contains("envelope = 'enriched'"))
        .stdout(predicate::str::contains("enriched_properties = 'source'"))
        .stdout(predicate::str::contains("resolved = '5s'"))
        .stdout(predicate::str::contains("#!/usr/bin/env bash").not())
        .stdout(predicate::str::contains("set -euo pipefail").not())
        .stdout(predicate::str::contains("COCKROACH_URL=").not())
        .stdout(predicate::str::contains("WEBHOOK_BASE_URL=").not())
        .stdout(predicate::str::contains("START_CURSOR=").not())
        .stdout(predicate::str::contains("tail -n +2").not())
        .stdout(predicate::str::contains("cut -d, -f1").not())
        .stdout(predicate::str::contains("printf '").not());

    let mut json_command = Command::cargo_bin("setup-sql").expect("setup-sql binary should exist");
    let json_assert = json_command
        .args(["emit-cockroach-sql", "--config"])
        .arg(fixture_path("valid-cockroach-setup-config.yml"))
        .args(["--format", "json"])
        .assert()
        .success();
    let json_output = String::from_utf8(json_assert.get_output().stdout.clone())
        .expect("emit-cockroach-sql json stdout should be utf-8");
    let payload: Value =
        serde_json::from_str(&json_output).expect("emit-cockroach-sql json should be valid JSON");
    let json_object = payload
        .as_object()
        .expect("emit-cockroach-sql json should be a top-level object");

    assert_eq!(
        json_object.len(),
        2,
        "emit-cockroach-sql json should emit one SQL string per source database",
    );
    assert!(
        json_object.contains_key("demo_a"),
        "emit-cockroach-sql json should key the SQL by source database",
    );
    assert!(
        json_object.contains_key("demo_b"),
        "emit-cockroach-sql json should key the SQL by source database",
    );

    let demo_a_sql = json_object
        .get("demo_a")
        .and_then(Value::as_str)
        .expect("demo_a sql should be a string");
    assert!(
        demo_a_sql.contains(
            "CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders"
        ),
        "demo_a json payload should contain the changefeed SQL string",
    );
    assert!(
        !demo_a_sql.contains("#!/usr/bin/env bash"),
        "json output must stay SQL-only and must not reintroduce shell artifacts",
    );
}

#[test]
fn emit_cockroach_sql_rejects_invalid_mapping_config() {
    let mut command = Command::cargo_bin("setup-sql").expect("setup-sql binary should exist");

    command
        .args(["emit-cockroach-sql", "--config"])
        .arg(fixture_path("invalid-cockroach-setup-config.yml"))
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "invalid config field `mappings[].id`",
        ))
        .stderr(predicate::str::contains("must be unique"));
}

#[test]
fn emit_postgres_grants_outputs_sql_only_text_and_json_from_minimal_destination_config() {
    let mut text_command = Command::cargo_bin("setup-sql").expect("setup-sql binary should exist");

    text_command
        .args(["emit-postgres-grants", "--config"])
        .arg(fixture_path("valid-postgres-grants-config.yml"))
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "GRANT CONNECT, TEMPORARY, CREATE ON DATABASE \"app_a\" TO \"migration_user_a\";",
        ))
        .stdout(predicate::str::contains(
            "GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE \"public\".\"customers\" TO \"migration_user_a\";",
        ))
        .stdout(predicate::str::contains("README.md").not())
        .stdout(predicate::str::contains("grants.sql").not())
        .stdout(predicate::str::contains("CREATE ROLE").not())
        .stdout(predicate::str::contains("SUPERUSER").not());

    let mut json_command = Command::cargo_bin("setup-sql").expect("setup-sql binary should exist");
    let json_assert = json_command
        .args(["emit-postgres-grants", "--config"])
        .arg(fixture_path("valid-postgres-grants-config.yml"))
        .args(["--format", "json"])
        .assert()
        .success();
    let json_output = String::from_utf8(json_assert.get_output().stdout.clone())
        .expect("emit-postgres-grants json stdout should be utf-8");
    let payload: Value =
        serde_json::from_str(&json_output).expect("emit-postgres-grants json should be valid JSON");
    let json_object = payload
        .as_object()
        .expect("emit-postgres-grants json should be a top-level object");

    assert_eq!(
        json_object.len(),
        2,
        "emit-postgres-grants json should emit one SQL string per destination database",
    );
    let app_a_sql = json_object
        .get("app_a")
        .and_then(Value::as_str)
        .expect("app_a sql should be a string");
    assert!(
        app_a_sql.contains(
            "GRANT CONNECT, TEMPORARY, CREATE ON DATABASE \"app_a\" TO \"migration_user_a\";"
        ),
        "app_a json payload should contain the explicit database grant SQL",
    );
}

#[test]
fn readme_setup_sql_cockroach_config_matches_its_canonical_fixture() {
    let readme = RepositoryReadme::load();
    let fixture_text = fs::read_to_string(fixture_path("readme-cockroach-setup-config.yml"))
        .expect("README Cockroach setup fixture should be readable");
    assert_eq!(
        readme.setup_sql_cockroach_yaml_block(),
        fixture_text.trim_end(),
        "README Cockroach setup YAML should match its canonical fixture"
    );
}
