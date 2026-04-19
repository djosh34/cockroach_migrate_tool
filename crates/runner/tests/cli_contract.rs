use assert_cmd::Command;
use predicates::prelude::{PredicateBooleanExt, predicate};

#[path = "support/runner_docker_contract.rs"]
mod runner_docker_contract_support;

use runner_docker_contract_support::RunnerDockerContract;

#[test]
fn runner_help_lists_only_destination_runtime_subcommands() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("render-postgres-setup"))
        .stdout(predicate::str::contains("validate-config"))
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("compare-schema").not())
        .stdout(predicate::str::contains("render-helper-plan").not())
        .stdout(predicate::str::contains("verify").not())
        .stdout(predicate::str::contains("cutover-readiness").not())
        .stdout(predicate::str::contains("--source-url").not())
        .stdout(predicate::str::contains("--cockroach-schema").not());
}

#[test]
fn runner_help_excludes_removed_source_only_flags() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--source-url").not())
        .stdout(predicate::str::contains("--cockroach-schema").not())
        .stdout(predicate::str::contains("--allow-tls-mode-disable").not());
}

#[test]
fn runner_help_covers_every_subcommand_documented_in_the_docker_quick_start() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");
    let assert = command.arg("--help").assert().success();
    let help_output = String::from_utf8(assert.get_output().stdout.clone())
        .expect("runner --help stdout should be utf-8");

    RunnerDockerContract::assert_cli_help_covers_documented_subcommands(&help_output);
}
