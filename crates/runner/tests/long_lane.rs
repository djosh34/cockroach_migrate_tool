use assert_cmd::Command;
use predicates::prelude::predicate;

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_keeps_runner_binary_contract_executable() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("validate-config"));
}
