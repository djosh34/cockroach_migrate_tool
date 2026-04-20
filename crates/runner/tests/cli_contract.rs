use assert_cmd::Command;

#[path = "support/operator_cli_surface.rs"]
mod operator_cli_surface_support;

use operator_cli_surface_support::OperatorCliSurface;

#[test]
fn runner_help_lists_only_destination_runtime_subcommands() {
    let contract = OperatorCliSurface::runner();
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");
    let assert = command.arg("--help").assert().success();
    let help_output =
        String::from_utf8(assert.get_output().stdout.clone()).expect("runner help should be utf-8");

    contract.assert_root_help_output(&help_output);
}

#[test]
fn runner_validate_config_help_stays_operator_focused() {
    let contract = OperatorCliSurface::runner();
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");
    let assert = command
        .args(
            contract
                .command_help("validate-config")
                .path_with_help_flag(),
        )
        .assert()
        .success();
    let help_output = String::from_utf8(assert.get_output().stdout.clone())
        .expect("runner validate-config help should be utf-8");

    contract
        .command_help("validate-config")
        .assert_help_output(&help_output);
}

#[test]
fn runner_run_help_stays_operator_focused() {
    let contract = OperatorCliSurface::runner();
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");
    let assert = command
        .args(contract.command_help("run").path_with_help_flag())
        .assert()
        .success();
    let help_output = String::from_utf8(assert.get_output().stdout.clone())
        .expect("runner run help should be utf-8");

    contract
        .command_help("run")
        .assert_help_output(&help_output);
}
