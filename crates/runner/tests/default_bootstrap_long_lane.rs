#[path = "support/e2e_harness.rs"]
mod e2e_harness;

use e2e_harness::DefaultBootstrapHarness;

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_bootstraps_a_default_cockroach_source_into_real_postgres_tables() {
    let harness = DefaultBootstrapHarness::start();

    harness.bootstrap_default_migration();
    harness.wait_for_destination_customers("1:alice@example.com,2:bob@example.com");
    harness.assert_explicit_source_bootstrap_commands();
    harness.assert_helper_shadow_customers(2);
    harness.verify_default_migration();
}
