use assert_cmd::Command;
use predicates::prelude::predicate;

#[test]
fn source_bootstrap_help_lists_render_bootstrap_script() {
    let mut command =
        Command::cargo_bin("source-bootstrap").expect("source-bootstrap binary should exist");

    command
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("render-bootstrap-script"));
}
