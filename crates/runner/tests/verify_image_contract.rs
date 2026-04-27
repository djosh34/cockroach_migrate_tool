#[path = "support/verify_docker_contract.rs"]
mod verify_docker_contract_support;
#[path = "support/verify_image_artifact_harness.rs"]
mod verify_image_artifact_harness_support;
#[path = "support/nix_image_artifact_harness.rs"]
mod nix_image_artifact_harness_support;

use serde_json::Value;
use verify_image_artifact_harness_support::VerifyImageArtifactHarness;

#[test]
#[ignore = "long lane"]
fn verify_image_builds_from_the_verify_slice() {
    let harness = VerifyImageArtifactHarness::start();

    harness.assert_image_exists();
}

#[test]
#[ignore = "long lane"]
fn verify_image_exposes_a_direct_verify_service_entrypoint() {
    let harness = VerifyImageArtifactHarness::start();

    verify_docker_contract_support::assert_image_entrypoint_is_direct_verify_surface(
        &harness.image_entrypoint_json(),
    );
}

#[test]
#[ignore = "long lane"]
fn verify_image_embeds_pgx_at_or_above_the_security_floor() {
    let harness = VerifyImageArtifactHarness::start();

    harness.assert_embedded_module_meets_minimum_version("github.com/jackc/pgx/v5", "v5.9.0");
}

#[test]
#[ignore = "long lane"]
fn verify_image_embeds_grpc_at_or_above_the_security_floor() {
    let harness = VerifyImageArtifactHarness::start();

    harness.assert_embedded_module_meets_minimum_version("google.golang.org/grpc", "v1.79.3");
}

#[test]
#[ignore = "long lane"]
fn verify_image_keeps_x_crypto_out_of_vulnerable_runtime_versions() {
    let harness = VerifyImageArtifactHarness::start();

    harness.assert_embedded_module_is_absent_or_meets_minimum_version(
        "golang.org/x/crypto",
        "v0.35.0",
    );
}

#[test]
#[ignore = "long lane"]
fn verify_image_runtime_filesystem_contains_only_the_binary_payload() {
    let harness = VerifyImageArtifactHarness::start();

    verify_docker_contract_support::assert_runtime_filesystem_is_minimal(
        &harness.exported_runtime_paths(),
    );
}

#[test]
#[ignore = "long lane"]
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
    assert_eq!(
        json_object.get("listener_mode").and_then(Value::as_str),
        Some("https+mtls"),
        "verify image json log must expose the effective listener mode",
    );
    assert_eq!(
        json_object.get("source_sslmode").and_then(Value::as_str),
        Some("verify-full"),
        "verify image json log must expose the source database sslmode",
    );
    assert_eq!(
        json_object
            .get("destination_sslmode")
            .and_then(Value::as_str),
        Some("verify-ca"),
        "verify image json log must expose the destination database sslmode",
    );
}
