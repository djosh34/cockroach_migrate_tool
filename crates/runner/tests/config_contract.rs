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
    r#"postgres:
  host: pg.example.internal
  port: 5432
  database: migration_db
  user: migration_user
  password: runner-secret
webhook:
  bind_addr: 127.0.0.1:8443
  tls_cert_path: /config/certs/server.crt
  tls_key_path: /config/certs/server.key
reconcile:
  interval_secs: 30
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
        .stdout(predicate::str::contains("config valid"));
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
        .stdout(predicate::str::contains(
            "pg.example.internal:5432/migration_db",
        ))
        .stdout(predicate::str::contains("127.0.0.1:8443"))
        .stdout(predicate::str::contains("30s"));
}
