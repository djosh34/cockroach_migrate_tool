use std::{fs, path::PathBuf};

use assert_cmd::Command;
use predicates::prelude::{PredicateBooleanExt, predicate};
use serde_json::Value;

#[path = "support/runner_public_contract.rs"]
mod runner_public_contract_support;

use runner_public_contract_support::RunnerPublicContract;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn validate_config_accepts_a_minimal_valid_yaml_file() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");
    let assert = command
        .args(["validate-config", "--config"])
        .arg(fixture_path("valid-runner-config.yml"))
        .assert()
        .success()
        .stdout(predicate::str::contains("config valid"))
        .stdout(predicate::str::contains("mappings=2"))
        .stdout(predicate::str::contains("webhook=127.0.0.1:8443"));
    let stdout = String::from_utf8(assert.get_output().stdout.clone())
        .expect("validate-config stdout should be utf-8");

    RunnerPublicContract::assert_text_excludes_removed_surface(
        &stdout,
        "validate-config stdout must not expose removed verify surface",
    );
    assert!(
        !stdout.contains("verify="),
        "validate-config stdout must not print a removed verify summary",
    );
}

#[test]
fn validate_config_supports_json_operator_logs() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");
    let assert = command
        .args(["validate-config", "--log-format", "json", "--config"])
        .arg(fixture_path("valid-runner-config.yml"))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone())
        .expect("validate-config stdout should be utf-8");
    let stderr = String::from_utf8(assert.get_output().stderr.clone())
        .expect("validate-config stderr should be utf-8");

    assert!(
        stdout.is_empty(),
        "json logging mode must keep validate-config stdout empty, got: {stdout:?}",
    );

    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "json logging mode must emit exactly one log line, got stderr: {stderr:?}",
    );

    let payload: Value =
        serde_json::from_str(lines[0]).expect("validate-config stderr must be valid JSON");
    let json_object = payload
        .as_object()
        .expect("validate-config stderr json must be an object");

    for key in ["timestamp", "level", "service", "event", "message"] {
        assert!(
            json_object.contains_key(key),
            "validate-config json log must include `{key}`: {payload}",
        );
    }

    assert_eq!(
        json_object.get("service").and_then(Value::as_str),
        Some("runner"),
        "validate-config json log must identify the runner service",
    );
    assert_eq!(
        json_object.get("event").and_then(Value::as_str),
        Some("config.validated"),
        "validate-config json log must expose the validation success event",
    );
    assert_eq!(
        json_object.get("mode").and_then(Value::as_str),
        Some("https"),
        "validate-config json log must expose the default webhook mode",
    );
    assert_eq!(
        json_object.get("tls").and_then(Value::as_str),
        Some("certs/server.crt+certs/server.key"),
        "validate-config json log must expose tls material when https is active",
    );
    assert!(
        !stderr.contains("config valid:"),
        "json logging mode must not fall back to the legacy plain-text summary: {stderr:?}",
    );
}

#[test]
fn validate_config_json_logs_omit_tls_for_http_webhook_mode() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8080
  mode: http
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
    destination:
      host: pg-a.example.internal
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
"#,
    )
    .expect("http webhook config should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");
    let assert = command
        .args(["validate-config", "--log-format", "json", "--config"])
        .arg(&config_path)
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone())
        .expect("validate-config stdout should be utf-8");
    let stderr = String::from_utf8(assert.get_output().stderr.clone())
        .expect("validate-config stderr should be utf-8");

    assert!(
        stdout.is_empty(),
        "json logging mode must keep validate-config stdout empty, got: {stdout:?}",
    );

    let payload: Value =
        serde_json::from_str(stderr.trim()).expect("validate-config stderr must be valid JSON");
    let json_object = payload
        .as_object()
        .expect("validate-config stderr json must be an object");

    assert_eq!(
        json_object.get("mode").and_then(Value::as_str),
        Some("http"),
        "validate-config json log must expose the selected http webhook mode",
    );
    assert!(
        !json_object.contains_key("tls"),
        "validate-config json log must omit tls when http mode is active: {payload}",
    );
}

#[test]
fn validate_config_fails_loudly_for_invalid_yaml_values() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(fixture_path("invalid-runner-config.yml"))
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "config: invalid config field `reconcile.interval_secs`: must be greater than zero",
        ));
}

#[test]
fn validate_config_rejects_duplicate_mapping_ids() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8443
  tls:
    cert_path: certs/server.crt
    key_path: certs/server.key
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
    destination:
      host: pg-a.example.internal
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
  - id: app-a
    source:
      database: demo_b
      tables:
        - public.invoices
    destination:
      host: pg-b.example.internal
      port: 5432
      database: app_b
      user: migration_user_b
      password: runner-secret-b
"#,
    )
    .expect("invalid config fixture should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "config: invalid config field `mappings.id`: must be unique",
        ));
}

#[test]
fn validate_config_rejects_duplicate_source_tables_within_a_mapping() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8443
  tls:
    cert_path: certs/server.crt
    key_path: certs/server.key
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.customers
    destination:
      host: pg-a.example.internal
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
"#,
    )
    .expect("invalid config fixture should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "config: invalid config field `mappings.source.tables`: must not contain duplicates",
        ));
}

#[test]
fn validate_config_rejects_unqualified_source_tables() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8443
  tls:
    cert_path: certs/server.crt
    key_path: certs/server.key
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - customers
    destination:
      host: pg-a.example.internal
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
"#,
    )
    .expect("invalid config fixture should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "config: invalid config field `mappings.source.tables`: entries must use schema.table",
        ));
}

#[test]
fn validate_config_accepts_a_mounted_config_directory_convention() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let mounted_config_dir = temp_dir.path().join("config");
    fs::create_dir_all(&mounted_config_dir).expect("mounted config dir should be created");
    let config_path = mounted_config_dir.join("runner.yml");
    fs::copy(fixture_path("container-runner-config.yml"), &config_path)
        .expect("mounted config fixture should be written");

    let config_path_label = config_path.display().to_string();
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "config={config_path_label}"
        )))
        .stdout(predicate::str::contains("mappings=2"))
        .stdout(predicate::str::contains("webhook=0.0.0.0:8443"))
        .stdout(predicate::str::contains("verify=").not())
        .stdout(predicate::str::contains(
            "tls=/config/certs/server.crt+/config/certs/server.key",
        ));
}

#[test]
fn validate_config_accepts_http_webhook_mode_without_tls() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8080
  mode: http
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
    destination:
      host: pg-a.example.internal
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
"#,
    )
    .expect("http webhook config should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("config valid"))
        .stdout(predicate::str::contains("webhook=127.0.0.1:8080"))
        .stdout(predicate::str::contains("mode=http"))
        .stdout(predicate::str::contains("tls=").not());
}

#[test]
fn validate_config_defaults_webhook_mode_to_https_when_tls_is_present() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(fixture_path("valid-runner-config.yml"))
        .assert()
        .success()
        .stdout(predicate::str::contains("mode=https"))
        .stdout(predicate::str::contains("tls=certs/server.crt+certs/server.key"));
}

#[test]
fn validate_config_rejects_https_webhook_mode_without_tls() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8443
  mode: https
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
    destination:
      host: pg-a.example.internal
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
"#,
    )
    .expect("https webhook config without tls should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "config: invalid config field `webhook.tls`: must be set when webhook.mode is `https`",
        ));
}

#[test]
fn validate_config_rejects_http_webhook_mode_with_tls_material() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8080
  mode: http
  tls:
    cert_path: certs/server.crt
    key_path: certs/server.key
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
    destination:
      host: pg-a.example.internal
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
"#,
    )
    .expect("http webhook config with tls should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "config: invalid config field `webhook.tls`: must not be set when webhook.mode is `http`",
        ));
}

#[test]
fn validate_config_accepts_an_explicit_postgresql_destination_target() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8443
  tls:
    cert_path: certs/server.crt
    key_path: certs/server.key
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
    destination:
      host: pg-a.example.internal
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
"#,
    )
    .expect("explicit postgres target config should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("config valid"));
}

#[test]
fn validate_config_accepts_a_postgresql_destination_url() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8443
  tls:
    cert_path: certs/server.crt
    key_path: certs/server.key
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
    destination:
      url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a
"#,
    )
    .expect("postgres destination url config should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("config valid"));
}

#[test]
fn validate_config_accepts_a_postgresql_destination_url_with_tls_query_parameters() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8443
  tls:
    cert_path: certs/server.crt
    key_path: certs/server.key
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
    destination:
      url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a?sslmode=verify-ca&sslrootcert=certs/postgres/ca.crt&sslcert=certs/postgres/client.crt&sslkey=certs/postgres/client.key
"#,
    )
    .expect("postgres destination url config with tls query parameters should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("config valid"));
}

#[test]
fn validate_config_rejects_mixing_destination_url_with_decomposed_fields() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8443
  tls:
    cert_path: certs/server.crt
    key_path: certs/server.key
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
    destination:
      url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a
      host: pg-a.example.internal
"#,
    )
    .expect("mixed destination config should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "config: invalid config field `mappings.destination`: `url` cannot be combined with `host`, `port`, `database`, `user`, `password`, or `tls`",
        ));
}

#[test]
fn validate_config_rejects_malformed_postgresql_destination_urls_with_parse_detail() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8443
  tls:
    cert_path: certs/server.crt
    key_path: certs/server.key
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
    destination:
      url: not-a-postgres-url
"#,
    )
    .expect("invalid destination url config should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "config: invalid config field `mappings.destination.url`: relative URL without a base",
        ));
}

#[test]
fn validate_config_accepts_a_secure_postgresql_destination_with_tls_material() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8443
  tls:
    cert_path: certs/server.crt
    key_path: certs/server.key
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
    destination:
      host: pg-a.example.internal
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
      tls:
        mode: verify-ca
        ca_cert_path: certs/postgres/ca.crt
        client_cert_path: certs/postgres/client.crt
        client_key_path: certs/postgres/client.key
"#,
    )
    .expect("secure postgres target config should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("config valid"));
}

#[test]
fn validate_config_rejects_legacy_verify_sections() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
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
        - public.customers
    destination:
      host: pg-a.example.internal
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
"#,
    )
    .expect("legacy config fixture should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "unknown field `verify`, expected one of `webhook`, `reconcile`, `mappings`",
        ));
}
