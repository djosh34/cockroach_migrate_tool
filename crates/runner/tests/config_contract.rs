use std::{fs, path::PathBuf};

use assert_cmd::Command;
use predicates::prelude::predicate;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn mounted_config_text() -> &'static str {
    r#"webhook:
  bind_addr: 127.0.0.1:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
reconcile:
  interval_secs: 30
verify:
  molt:
    command: molt
    report_dir: /work/molt-verify
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.orders
    destination:
      connection:
        host: pg-a.example.internal
        port: 5432
        database: app_a
        user: migration_user_a
        password: runner-secret-a
  - id: app-b
    source:
      database: demo_b
      tables:
        - public.invoices
    destination:
      connection:
        host: pg-b.example.internal
        port: 5432
        database: app_b
        user: migration_user_b
        password: runner-secret-b
"#
}

#[test]
fn validate_config_accepts_a_minimal_valid_yaml_file() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(fixture_path("valid-runner-config.yml"))
        .assert()
        .success()
        .stdout(predicate::str::contains("config valid"))
        .stdout(predicate::str::contains("mappings=2"))
        .stdout(predicate::str::contains("verify=molt@/tmp/molt"));
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
      connection:
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
      connection:
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
        - public.customers
    destination:
      connection:
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
fn validate_config_accepts_a_mounted_config_directory_convention() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let mounted_config_dir = temp_dir.path().join("config");
    fs::create_dir_all(&mounted_config_dir).expect("mounted config dir should be created");
    let config_path = mounted_config_dir.join("runner.yml");
    fs::write(&config_path, mounted_config_text()).expect("mounted config fixture should be written");

    let config_path_label = config_path.display().to_string();
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("config={config_path_label}")))
        .stdout(predicate::str::contains("mappings=2"))
        .stdout(predicate::str::contains("verify=molt@/work/molt-verify"))
        .stdout(predicate::str::contains(
            "tls=/config/certs/server.crt+/config/certs/server.key",
        ));
}

#[test]
fn run_reports_the_wired_runner_modules_from_valid_config() {
    let config_path = fixture_path("valid-runner-config.yml");
    let config_path_label = config_path.display().to_string();
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["run", "--config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("runner ready"))
        .stdout(predicate::str::contains(format!("config={config_path_label}")))
        .stdout(predicate::str::contains("mappings=2"))
        .stdout(predicate::str::contains(
            "labels=app-a=demo_a(2 tables)->pg-a.example.internal:5432/app_a,app-b=demo_b(1 tables)->pg-b.example.internal:5432/app_b",
        ))
        .stdout(predicate::str::contains("verify=molt@/tmp/molt"))
        .stdout(predicate::str::contains("127.0.0.1:8443"))
        .stdout(predicate::str::contains("30s"));
}
