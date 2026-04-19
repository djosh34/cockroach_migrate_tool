#[path = "support/verify_docker_contract.rs"]
mod verify_docker_contract_support;
#[path = "support/verify_image_artifact_harness.rs"]
mod verify_image_artifact_harness_support;

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
    contract.assert_verify_help_output(&harness.help_output());
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
