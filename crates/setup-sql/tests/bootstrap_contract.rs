use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::{PredicateBooleanExt, predicate};
use serde_json::Value;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

const FIXTURE_CA_CERT_QUERY: &str = "ZHVtbXktY2EK";
const CURSOR_PLACEHOLDER: &str = "__CHANGEFEED_CURSOR__";

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
            "SELECT cluster_logical_timestamp() AS changefeed_cursor;",
        ))
        .stdout(predicate::str::contains(format!(
            "-- Replace {CURSOR_PLACEHOLDER} below with the decimal cursor returned above before running the CREATE CHANGEFEED statement."
        )))
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
            "cursor = '{CURSOR_PLACEHOLDER}'"
        )))
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
        demo_a_sql.contains("SELECT cluster_logical_timestamp() AS changefeed_cursor;"),
        "demo_a json payload should preserve the explicit cursor capture step",
    );
    assert!(
        demo_a_sql.contains(&format!("cursor = '{CURSOR_PLACEHOLDER}'")),
        "demo_a json payload should keep the explicit cursor handoff in the changefeed SQL",
    );
    assert!(
        demo_a_sql
            .contains("CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders"),
        "demo_a json payload should contain the changefeed SQL string",
    );
    assert!(
        !demo_a_sql.contains("#!/usr/bin/env bash"),
        "json output must stay SQL-only and must not reintroduce shell artifacts",
    );
}

#[test]
fn emit_cockroach_sql_supports_json_operator_logs_without_mixing_payload_output() {
    let mut command = Command::cargo_bin("setup-sql").expect("setup-sql binary should exist");
    let assert = command
        .args(["emit-cockroach-sql", "--log-format", "json", "--config"])
        .arg(fixture_path("valid-cockroach-setup-config.yml"))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone())
        .expect("emit-cockroach-sql stdout should be utf-8");
    let stderr = String::from_utf8(assert.get_output().stderr.clone())
        .expect("emit-cockroach-sql stderr should be utf-8");

    assert!(
        stdout.starts_with("-- Source bootstrap SQL\n"),
        "emit-cockroach-sql must keep stdout reserved for the SQL payload, got: {stdout:?}",
    );

    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "json logging mode must emit exactly one stderr log line, got: {stderr:?}",
    );

    let payload: Value =
        serde_json::from_str(lines[0]).expect("emit-cockroach-sql stderr must be valid JSON");
    let json_object = payload
        .as_object()
        .expect("emit-cockroach-sql stderr json must be an object");

    for key in ["timestamp", "level", "service", "event", "message"] {
        assert!(
            json_object.contains_key(key),
            "emit-cockroach-sql json log must include `{key}`: {payload}",
        );
    }

    assert_eq!(
        json_object.get("service").and_then(Value::as_str),
        Some("setup-sql"),
        "emit-cockroach-sql json log must identify the setup-sql service",
    );
    assert_eq!(
        json_object.get("event").and_then(Value::as_str),
        Some("sql.emitted"),
        "emit-cockroach-sql json log must identify the SQL emission event",
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
fn emit_cockroach_sql_reports_invalid_config_as_a_json_error_event() {
    let mut command = Command::cargo_bin("setup-sql").expect("setup-sql binary should exist");
    let assert = command
        .args(["emit-cockroach-sql", "--log-format", "json", "--config"])
        .arg(fixture_path("invalid-cockroach-setup-config.yml"))
        .assert()
        .failure();

    let stdout = String::from_utf8(assert.get_output().stdout.clone())
        .expect("emit-cockroach-sql stdout should be utf-8");
    let stderr = String::from_utf8(assert.get_output().stderr.clone())
        .expect("emit-cockroach-sql stderr should be utf-8");

    assert!(
        stdout.is_empty(),
        "invalid config must not emit SQL payload on stdout, got: {stdout:?}",
    );

    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "json logging mode must emit exactly one stderr log line for invalid config, got: {stderr:?}",
    );

    let payload: Value =
        serde_json::from_str(lines[0]).expect("emit-cockroach-sql stderr must be valid JSON");
    let json_object = payload
        .as_object()
        .expect("emit-cockroach-sql stderr json must be an object");

    for key in ["timestamp", "level", "service", "event", "message"] {
        assert!(
            json_object.contains_key(key),
            "emit-cockroach-sql json error log must include `{key}`: {payload}",
        );
    }

    assert_eq!(
        json_object.get("level").and_then(Value::as_str),
        Some("error"),
        "invalid config must log at error level",
    );
    assert_eq!(
        json_object.get("service").and_then(Value::as_str),
        Some("setup-sql"),
        "invalid config json log must identify the setup-sql service",
    );
    assert_eq!(
        json_object.get("event").and_then(Value::as_str),
        Some("command.failed"),
        "invalid config must surface the command failure event",
    );

    let message = json_object
        .get("message")
        .and_then(Value::as_str)
        .expect("invalid config json error must expose the error message");
    assert!(
        message.contains("invalid config field `mappings[].id`"),
        "invalid config json error must retain the field-level validation detail, got: {message:?}",
    );
    assert!(
        message.contains("must be unique"),
        "invalid config json error must retain the explicit failure reason, got: {message:?}",
    );
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
            "GRANT CONNECT, CREATE ON DATABASE \"app_a\" TO \"migration_user_a\";",
        ))
        .stdout(predicate::str::contains(
            "GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE \"public\".\"customers\" TO \"migration_user_a\";",
        ))
        .stdout(predicate::str::contains(
            "GRANT CONNECT, CREATE ON DATABASE \"app_b\" TO \"migration_user_b\";",
        ))
        .stdout(predicate::str::contains("README.md").not())
        .stdout(predicate::str::contains("grants.sql").not())
        .stdout(predicate::str::contains("CREATE ROLE").not())
        .stdout(predicate::str::contains("SUPERUSER").not())
        .stdout(predicate::str::contains("TEMPORARY").not())
        .stdout(predicate::str::contains("ALL PRIVILEGES").not())
        .stdout(predicate::str::contains("ALL TABLES IN SCHEMA").not());

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
        app_a_sql.contains("GRANT CONNECT, CREATE ON DATABASE \"app_a\" TO \"migration_user_a\";"),
        "app_a json payload should contain the explicit database grant SQL",
    );
    assert!(
        !app_a_sql.contains("TEMPORARY"),
        "app_a json payload must not include temporary-table privileges",
    );
    assert!(
        !app_a_sql.contains("ALL PRIVILEGES"),
        "app_a json payload must not broaden the contract with blanket database grants",
    );
    assert!(
        !app_a_sql.contains("ALL TABLES IN SCHEMA"),
        "app_a json payload must not emit schema-wide blanket table grants",
    );
}
