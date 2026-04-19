use assert_cmd::Command;
use predicates::prelude::predicate;

#[path = "support/runner_docker_contract.rs"]
mod runner_docker_contract_support;

use runner_docker_contract_support::RunnerDockerContract;

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
        .stdout(predicate::str::contains("cutover-readiness"))
        .stdout(predicate::str::contains("run"));
}

#[test]
fn cutover_readiness_help_describes_the_operator_contract() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["cutover-readiness", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Determine whether a mapping has drained to zero and is ready for final cutover",
        ))
        .stdout(predicate::str::contains("--source-url"))
        .stdout(predicate::str::contains("--allow-tls-mode-disable"));
}

#[test]
fn runner_help_covers_every_subcommand_documented_in_the_docker_quick_start() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");
    let assert = command.arg("--help").assert().success();
    let help_output = String::from_utf8(assert.get_output().stdout.clone())
        .expect("runner --help stdout should be utf-8");

    RunnerDockerContract::assert_cli_help_covers_documented_subcommands(&help_output);
}
