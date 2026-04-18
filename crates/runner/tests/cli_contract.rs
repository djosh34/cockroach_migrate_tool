use assert_cmd::Command;
use predicates::prelude::predicate;

#[test]
fn runner_help_lists_the_core_subcommands() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("compare-schema"))
        .stdout(predicate::str::contains("render-helper-plan"))
        .stdout(predicate::str::contains("render-postgres-setup"))
        .stdout(predicate::str::contains("validate-config"))
        .stdout(predicate::str::contains("verify"))
        .stdout(predicate::str::contains("run"));
}
