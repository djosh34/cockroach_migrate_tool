use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::predicate;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
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
fn run_reports_the_wired_runner_modules_from_valid_config() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["run", "--config"])
        .arg(fixture_path("valid-runner-config.yml"))
        .assert()
        .success()
        .stdout(predicate::str::contains("runner ready"))
        .stdout(predicate::str::contains(
            "pg.example.internal:5432/migration_db",
        ))
        .stdout(predicate::str::contains("127.0.0.1:8443"))
        .stdout(predicate::str::contains("30s"));
}
