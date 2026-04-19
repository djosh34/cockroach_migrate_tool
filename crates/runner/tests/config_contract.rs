use std::{fs, path::PathBuf};

use assert_cmd::Command;
use predicates::prelude::{PredicateBooleanExt, predicate};

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
