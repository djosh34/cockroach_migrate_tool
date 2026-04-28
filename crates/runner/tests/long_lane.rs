#[path = "support/nix_image_artifact_harness.rs"]
mod nix_image_artifact_harness_support;
#[path = "support/runner_docker_contract.rs"]
mod runner_docker_contract;
#[path = "support/runner_image_harness.rs"]
mod runner_image_harness;
#[path = "support/runner_public_contract.rs"]
mod runner_public_contract;

use runner_docker_contract::RunnerDockerContract;
use runner_image_harness::RunnerImageHarness;
use runner_public_contract::RunnerPublicContract;

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_builds_and_runs_the_single_binary_runner_image_against_real_postgres() {
    let harness = RunnerImageHarness::start();

    RunnerDockerContract::assert_image_entrypoint_is_direct_runner(
        &harness.image_entrypoint_json(),
    );

    let validate_stdout = harness.validate_mounted_config();
    RunnerPublicContract::assert_text_excludes_removed_surface(
        &validate_stdout,
        "runner image validate-config output must not expose removed verify surface",
    );
    assert!(
        !validate_stdout.contains("verify="),
        "runner image validate-config output must not print a removed verify summary",
    );
    assert!(validate_stdout.contains("config=/config/container-runner-config.yml"));
    assert!(validate_stdout.contains("mappings=2"));
    assert!(validate_stdout.contains("tls=/config/certs/server.crt+/config/certs/server.key"));

    harness.start_runner_container();
    harness.wait_for_runner_health();

    assert_eq!(
        harness.helper_tables("app_a").trim(),
        "app-a__public__customers,app-a__public__orders,stream_state,table_sync_state"
    );
    assert_eq!(
        harness.helper_tables("app_b").trim(),
        "app-b__public__invoices,stream_state,table_sync_state"
    );
}
