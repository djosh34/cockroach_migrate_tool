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
fn create_changefeed_reports_the_bootstrap_plan_from_valid_config() {
    let mut command =
        Command::cargo_bin("source-bootstrap").expect("source-bootstrap binary should exist");

    command
        .args(["create-changefeed", "--config"])
        .arg(fixture_path("valid-source-bootstrap-config.yml"))
        .assert()
        .success()
        .stdout(predicate::str::contains("bootstrap plan ready"))
        .stdout(predicate::str::contains("public.accounts, public.orders"))
        .stdout(predicate::str::contains(
            "https://runner.example.internal:8443/events",
        ));
}
