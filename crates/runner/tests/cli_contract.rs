use assert_cmd::Command;

#[path = "support/runner_docker_contract.rs"]
mod runner_docker_contract_support;
#[path = "support/runner_public_contract.rs"]
mod runner_public_contract_support;

use runner_docker_contract_support::RunnerDockerContract;
use runner_public_contract_support::RunnerPublicContract;

#[test]
fn runner_help_lists_only_destination_runtime_subcommands() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");
    let assert = command.arg("--help").assert().success();
    let help_output =
        String::from_utf8(assert.get_output().stdout.clone()).expect("runner help should be utf-8");

    for subcommand in RunnerDockerContract::documented_subcommands() {
        assert!(
            help_output.contains(subcommand),
            "runner --help must include runtime subcommand `{subcommand}`",
        );
    }
    RunnerPublicContract::assert_text_excludes_removed_surface(
        &help_output,
        "runner --help must not expose removed verify surface",
    );
}

#[test]
fn runner_help_excludes_removed_source_only_flags() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");
    let assert = command.arg("--help").assert().success();
    let help_output =
        String::from_utf8(assert.get_output().stdout.clone()).expect("runner help should be utf-8");

    RunnerPublicContract::assert_text_excludes_removed_surface(
        &help_output,
        "runner --help must not expose removed source-only flags",
    );
}

#[test]
fn runner_help_covers_every_subcommand_documented_in_the_docker_quick_start() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");
    let assert = command.arg("--help").assert().success();
    let help_output = String::from_utf8(assert.get_output().stdout.clone())
        .expect("runner --help stdout should be utf-8");

    RunnerDockerContract::assert_cli_help_covers_documented_subcommands(&help_output);
}
