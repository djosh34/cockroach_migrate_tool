use assert_cmd::Command;

#[path = "../../runner/tests/support/operator_cli_surface.rs"]
mod operator_cli_surface_support;

use operator_cli_surface_support::OperatorCliSurface;

#[test]
fn setup_sql_help_lists_exactly_the_two_manual_sql_subcommands() {
    let mut command = Command::cargo_bin("setup-sql").expect("setup-sql binary should exist");
    let assert = command.arg("--help").assert().success();
    let help_output = String::from_utf8(assert.get_output().stdout.clone())
        .expect("setup-sql help should be utf-8");

    OperatorCliSurface::setup_sql().assert_root_help_output(&help_output);
}

#[test]
fn setup_sql_emit_cockroach_sql_help_stays_operator_focused() {
    let contract = OperatorCliSurface::setup_sql();
    let mut command = Command::cargo_bin("setup-sql").expect("setup-sql binary should exist");
    let assert = command
        .args(
            contract
                .command_help("emit-cockroach-sql")
                .path_with_help_flag(),
        )
        .assert()
        .success();
    let help_output = String::from_utf8(assert.get_output().stdout.clone())
        .expect("setup-sql emit-cockroach-sql help should be utf-8");

    contract
        .command_help("emit-cockroach-sql")
        .assert_help_output(&help_output);
}

#[test]
fn setup_sql_emit_postgres_grants_help_stays_operator_focused() {
    let contract = OperatorCliSurface::setup_sql();
    let mut command = Command::cargo_bin("setup-sql").expect("setup-sql binary should exist");
    let assert = command
        .args(
            contract
                .command_help("emit-postgres-grants")
                .path_with_help_flag(),
        )
        .assert()
        .success();
    let help_output = String::from_utf8(assert.get_output().stdout.clone())
        .expect("setup-sql emit-postgres-grants help should be utf-8");

    contract
        .command_help("emit-postgres-grants")
        .assert_help_output(&help_output);
}
