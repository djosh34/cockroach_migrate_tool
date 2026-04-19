use assert_cmd::Command;
use predicates::prelude::{PredicateBooleanExt, predicate};

#[test]
fn source_bootstrap_help_lists_render_bootstrap_sql_only() {
    let mut command =
        Command::cargo_bin("source-bootstrap").expect("source-bootstrap binary should exist");

    command
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("render-bootstrap-sql"))
        .stdout(predicate::str::contains("render-bootstrap-script").not());
}
