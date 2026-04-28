#[path = "support/docker_image_container_harness.rs"]
mod docker_image_container_harness_support;
#[path = "support/nix_image_artifact_harness.rs"]
mod nix_image_artifact_harness_support;
#[path = "support/runner_docker_contract.rs"]
mod runner_docker_contract_support;
#[path = "support/runner_image_artifact_harness.rs"]
mod runner_image_artifact_harness_support;

use runner_docker_contract_support::RunnerDockerContract;
use runner_image_artifact_harness_support::RunnerImageArtifactHarness;
use serde_json::Value;

#[test]
#[ignore = "long lane"]
fn runner_image_builds_from_the_root_runtime_slice() {
    let harness = RunnerImageArtifactHarness::start();

    harness.assert_image_exists();
}

#[test]
#[ignore = "long lane"]
fn runner_image_exposes_a_direct_runtime_only_entrypoint() {
    let harness = RunnerImageArtifactHarness::start();

    RunnerDockerContract::assert_image_entrypoint_is_direct_runner(
        &harness.image_entrypoint_json(),
    );
}

#[test]
#[ignore = "long lane"]
fn runner_image_runtime_filesystem_contains_only_the_runner_payload() {
    let harness = RunnerImageArtifactHarness::start();

    RunnerDockerContract::assert_runtime_filesystem_is_minimal(&harness.exported_runtime_paths());
}

#[test]
#[ignore = "long lane"]
fn runner_image_validate_config_supports_json_operator_logs() {
    let harness = RunnerImageArtifactHarness::start();
    let (stdout, stderr) = harness.validate_config_json_logs();

    assert!(
        stdout.is_empty(),
        "runner image json logging mode must keep validate-config stdout empty, got: {stdout:?}",
    );

    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "runner image json logging mode must emit exactly one log line, got: {stderr:?}",
    );

    let payload: Value =
        serde_json::from_str(lines[0]).expect("runner image stderr log should be valid json");
    let json_object = payload
        .as_object()
        .expect("runner image stderr log should be a json object");

    for key in ["timestamp", "level", "service", "event", "message"] {
        assert!(
            json_object.contains_key(key),
            "runner image json log must include `{key}`: {payload}",
        );
    }
    assert_eq!(
        json_object.get("service").and_then(Value::as_str),
        Some("runner"),
        "runner image json log must identify the runner service",
    );
    assert_eq!(
        json_object.get("event").and_then(Value::as_str),
        Some("config.validated"),
        "runner image json log must expose the validation success event",
    );
}
