use assert_cmd::Command;
use predicates::prelude::{PredicateBooleanExt, predicate};

#[test]
fn setup_sql_help_lists_exactly_the_two_manual_sql_subcommands() {
    let mut command = Command::cargo_bin("setup-sql").expect("setup-sql binary should exist");

    command
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("emit-cockroach-sql"))
        .stdout(predicate::str::contains("emit-postgres-grants"))
        .stdout(predicate::str::contains("render-bootstrap-sql").not())
        .stdout(predicate::str::contains("render-postgres-setup").not())
        .stdout(predicate::str::contains("run").not());
}
