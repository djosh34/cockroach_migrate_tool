#[path = "support/operator_cli_surface.rs"]
mod operator_cli_surface_support;
#[path = "support/verify_docker_contract.rs"]
mod verify_docker_contract_support;
#[path = "support/verify_image_artifact_harness.rs"]
mod verify_image_artifact_harness_support;

use operator_cli_surface_support::OperatorCliSurface;
use serde_json::Value;
use verify_docker_contract_support::VerifyDockerContract;
use verify_image_artifact_harness_support::VerifyImageArtifactHarness;

#[test]
fn verify_image_dockerfile_lives_in_the_verify_slice_and_uses_a_scratch_runtime() {
    let contract = VerifyDockerContract::load();

    contract.assert_verify_slice_owns_the_dockerfile();
    contract.assert_runtime_is_scratch_with_a_direct_binary_entrypoint();
}

#[test]
fn verify_image_builds_from_the_verify_slice() {
    let harness = VerifyImageArtifactHarness::start();

    harness.assert_image_exists();
}

#[test]
fn verify_image_exposes_only_the_verify_command_surface() {
    let harness = VerifyImageArtifactHarness::start();
    let contract = VerifyDockerContract::load();

    contract.assert_image_entrypoint_is_direct_verify_surface(&harness.image_entrypoint_json());
    OperatorCliSurface::verify_service_image().assert_root_help_output(&harness.help_output());
}

#[test]
fn verify_image_runtime_filesystem_contains_only_the_binary_payload() {
    let harness = VerifyImageArtifactHarness::start();
    let contract = VerifyDockerContract::load();

    contract.assert_runtime_filesystem_is_minimal(&harness.exported_runtime_paths());
}

#[test]
fn verify_image_dockerfile_stays_scoped_to_the_verify_slice() {
    let contract = VerifyDockerContract::load();

    contract.assert_build_context_stays_within_the_verify_slice();
}

#[test]
fn verify_image_dockerfile_separates_go_module_and_source_cache_layers() {
    let contract = VerifyDockerContract::load();

    contract.assert_dockerfile_separates_go_module_and_source_cache_layers();
}

#[test]
fn verify_image_validate_config_supports_json_operator_logs() {
    let harness = VerifyImageArtifactHarness::start();
    let (stdout, stderr) = harness.validate_config_json_logs();

    assert!(
        stdout.is_empty(),
        "verify image json logging mode must keep validate-config stdout empty, got: {stdout:?}",
    );

    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "verify image json logging mode must emit exactly one log line, got: {stderr:?}",
    );

    let payload: Value =
        serde_json::from_str(lines[0]).expect("verify image stderr log should be valid json");
    let json_object = payload
        .as_object()
        .expect("verify image stderr log should be a json object");

    for key in ["timestamp", "level", "service", "event", "message"] {
        assert!(
            json_object.contains_key(key),
            "verify image json log must include `{key}`: {payload}",
        );
    }
    assert_eq!(
        json_object.get("service").and_then(Value::as_str),
        Some("verify"),
        "verify image json log must identify the verify service",
    );
    assert_eq!(
        json_object.get("event").and_then(Value::as_str),
        Some("config.validated"),
        "verify image json log must expose the validation success event",
    );
}
