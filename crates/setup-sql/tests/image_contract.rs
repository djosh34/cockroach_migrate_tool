#[path = "support/source_bootstrap_image_contract.rs"]
mod source_bootstrap_image_contract_support;
#[path = "support/source_bootstrap_image_harness.rs"]
mod source_bootstrap_image_harness_support;

use source_bootstrap_image_contract_support::SourceBootstrapImageContract;
use source_bootstrap_image_harness_support::SourceBootstrapImageHarness;

#[test]
fn setup_sql_image_dockerfile_lives_in_the_setup_slice() {
    let contract = SourceBootstrapImageContract::load();

    contract.assert_setup_slice_owns_the_dockerfile();
    contract.assert_runtime_is_scratch_with_a_direct_binary_entrypoint();
}

#[test]
fn setup_sql_image_dockerfile_uses_dependency_first_rust_cache_layers() {
    let contract = SourceBootstrapImageContract::load();

    contract.assert_dockerfile_uses_dependency_first_rust_cache_layers();
}

#[test]
fn setup_sql_image_runs_emit_cockroach_sql_from_a_mounted_config() {
    let harness = SourceBootstrapImageHarness::start();
    let contract = SourceBootstrapImageContract::load();

    contract.assert_image_entrypoint_is_direct_setup_sql(&harness.image_entrypoint_json());
    harness.assert_emit_cockroach_sql_output();
}
