#[path = "support/runner_docker_contract.rs"]
mod runner_docker_contract_support;
#[path = "support/runner_image_artifact_harness.rs"]
mod runner_image_artifact_harness_support;
#[path = "support/runner_public_contract.rs"]
mod runner_public_contract_support;

use runner_docker_contract_support::RunnerDockerContract;
use runner_image_artifact_harness_support::RunnerImageArtifactHarness;
use runner_public_contract_support::RunnerPublicContract;

#[test]
fn runner_image_builds_from_the_root_runtime_slice() {
    let harness = RunnerImageArtifactHarness::start();

    harness.assert_image_exists();
}

#[test]
fn runner_image_exposes_a_direct_runtime_only_entrypoint() {
    let harness = RunnerImageArtifactHarness::start();

    RunnerDockerContract::assert_image_entrypoint_is_direct_runner(
        &harness.image_entrypoint_json(),
    );
}

#[test]
fn runner_image_help_surface_stays_runtime_only() {
    let harness = RunnerImageArtifactHarness::start();
    let help_output = harness.help_output();

    RunnerDockerContract::assert_cli_help_covers_documented_subcommands(&help_output);
    RunnerPublicContract::assert_text_excludes_removed_surface(
        &help_output,
        "runner image --help must not expose removed verify or source-only surface",
    );
}

#[test]
fn runner_image_runtime_filesystem_contains_only_the_runner_payload() {
    let harness = RunnerImageArtifactHarness::start();

    RunnerDockerContract::assert_runtime_filesystem_is_minimal(&harness.exported_runtime_paths());
}
