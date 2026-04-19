#[path = "support/composite_pk_exclusion_harness.rs"]
mod composite_pk_exclusion_harness;
#[path = "support/default_bootstrap_harness.rs"]
mod default_bootstrap_harness;
#[path = "support/e2e_harness.rs"]
mod e2e_harness;
#[path = "support/e2e_integrity.rs"]
mod e2e_integrity;
#[path = "support/multi_mapping_harness.rs"]
mod multi_mapping_harness;
#[path = "support/webhook_chaos_gateway.rs"]
mod webhook_chaos_gateway;

use std::{thread, time::Duration};

use composite_pk_exclusion_harness::CompositePkExclusionHarness;
use default_bootstrap_harness::DefaultBootstrapHarness;
use e2e_harness::{CdcE2eHarness, CdcE2eHarnessConfig};
use multi_mapping_harness::MultiMappingHarness;

const FK_HEAVY_SOURCE_SETUP_SQL: &str = r#"
CREATE DATABASE demo_a;
USE demo_a;
CREATE TABLE public.parents (
    id INT8 PRIMARY KEY,
    name STRING NOT NULL
);
CREATE TABLE public.children (
    id INT8 PRIMARY KEY,
    parent_id INT8 NOT NULL REFERENCES public.parents(id),
    name STRING NOT NULL
);
CREATE TABLE public.grandchildren (
    id INT8 PRIMARY KEY,
    child_id INT8 NOT NULL REFERENCES public.children(id),
    name STRING NOT NULL
);
INSERT INTO public.parents (id, name) VALUES
    (1, 'alpha parent'),
    (2, 'beta parent');
INSERT INTO public.children (id, parent_id, name) VALUES
    (10, 1, 'alpha child'),
    (20, 2, 'beta child');
INSERT INTO public.grandchildren (id, child_id, name) VALUES
    (100, 10, 'alpha grandchild'),
    (200, 20, 'beta grandchild');
"#;

const FK_HEAVY_DESTINATION_SETUP_SQL: &str = r#"
CREATE TABLE public.parents (
    id bigint PRIMARY KEY,
    name text NOT NULL
);
CREATE TABLE public.children (
    id bigint PRIMARY KEY,
    parent_id bigint NOT NULL REFERENCES public.parents(id),
    name text NOT NULL
);
CREATE TABLE public.grandchildren (
    id bigint PRIMARY KEY,
    child_id bigint NOT NULL REFERENCES public.children(id),
    name text NOT NULL
);
"#;

const FK_HEAVY_SNAPSHOT_SQL: &str = r#"
SELECT string_agg(entry, ',' ORDER BY entry)
FROM (
    SELECT 'p:' || id::text || ':' || name AS entry FROM public.parents
    UNION ALL
    SELECT 'c:' || id::text || ':' || parent_id::text || ':' || name FROM public.children
    UNION ALL
    SELECT 'g:' || id::text || ':' || child_id::text || ':' || name FROM public.grandchildren
) snapshot;
"#;

const FK_HEAVY_CONSTRAINTS: &str = "children:children_parent_id_fkey:FOREIGN KEY,children:children_pkey:PRIMARY KEY,grandchildren:grandchildren_child_id_fkey:FOREIGN KEY,grandchildren:grandchildren_pkey:PRIMARY KEY,parents:parents_pkey:PRIMARY KEY";

fn apply_fk_heavy_live_source_changes(harness: &CdcE2eHarness) {
    harness.apply_source_workload_batch(
        r#"
INSERT INTO public.parents (id, name) VALUES (3, 'gamma parent');
INSERT INTO public.children (id, parent_id, name) VALUES (30, 3, 'gamma child');
INSERT INTO public.grandchildren (id, child_id, name) VALUES (300, 30, 'gamma grandchild');
UPDATE public.parents SET name = 'alpha parent updated' WHERE id = 1;
DELETE FROM public.grandchildren WHERE id = 200;
DELETE FROM public.children WHERE id = 20;
DELETE FROM public.parents WHERE id = 2;
"#,
    );
}

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_bootstraps_a_default_cockroach_source_into_real_postgres_tables() {
    let harness = DefaultBootstrapHarness::start();

    harness.bootstrap_default_migration();
    harness.wait_for_destination_customers("1:alice@example.com,2:bob@example.com");
    harness.assert_explicit_source_bootstrap_commands();
    harness.assert_helper_shadow_customers(2);
    harness
        .runtime_shape_audit()
        .assert_honest_default_runtime_shape();
}

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_proves_customer_live_update_flows_through_webhook_then_helper_then_reconcile()
{
    let harness =
        DefaultBootstrapHarness::start_with_observed_webhook_gateway_and_reconcile_interval(30);

    harness.bootstrap_default_migration();
    harness.wait_for_destination_customers("1:alice@example.com,2:bob@example.com");
    let baseline = harness.customer_tracking_progress();
    harness.update_source_customer_email(1, "alice+live-path@example.com");
    let live_update = harness.wait_for_customer_update_received_before_reconcile(
        &baseline,
        "1:alice+live-path@example.com,2:bob@example.com",
        "1:alice@example.com,2:bob@example.com",
        "alice+live-path@example.com",
    );
    harness.wait_for_customer_update_reconcile(
        &live_update,
        "1:alice+live-path@example.com,2:bob@example.com",
    );
    harness
        .post_setup_source_audit()
        .assert_honest_workload_only(1);
}

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_retries_customer_update_after_external_http_500_and_converges() {
    let harness = DefaultBootstrapHarness::start_with_observed_webhook_gateway();

    harness.bootstrap_default_migration();
    harness.wait_for_destination_customers("1:alice@example.com,2:bob@example.com");
    harness.arm_single_external_http_500_for_customer_email("alice+retry@example.com");
    harness.update_source_customer_email(1, "alice+retry@example.com");
    harness.wait_for_duplicate_customer_delivery("alice+retry@example.com");
    harness.wait_for_helper_shadow_customers("1:alice+retry@example.com,2:bob@example.com");
    harness.assert_helper_shadow_customers(2);
    harness.wait_for_destination_customers("1:alice+retry@example.com,2:bob@example.com");
    harness.assert_destination_customers_stable(
        "1:alice+retry@example.com,2:bob@example.com",
        Duration::from_secs(3),
    );
}

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_recovers_from_external_network_fault_and_converges() {
    let harness = DefaultBootstrapHarness::start_with_observed_webhook_gateway();

    harness.bootstrap_default_migration();
    harness.wait_for_destination_customers("1:alice@example.com,2:bob@example.com");
    harness
        .arm_single_external_transport_disconnect_for_customer_email("alice+network@example.com");
    harness.update_source_customer_email(1, "alice+network@example.com");
    harness.wait_for_gateway_transport_abort_then_success_for_customer_email(
        "alice+network@example.com",
    );
    harness.wait_for_helper_shadow_customers("1:alice+network@example.com,2:bob@example.com");
    harness.assert_helper_shadow_customers(2);
    harness.wait_for_destination_customers("1:alice+network@example.com,2:bob@example.com");
    harness.assert_helper_shadow_customers_stable(
        "1:alice+network@example.com,2:bob@example.com",
        Duration::from_secs(3),
    );
    harness.assert_destination_customers_stable(
        "1:alice+network@example.com,2:bob@example.com",
        Duration::from_secs(3),
    );
    harness.update_source_customer_email(2, "bob+recovered@example.com");
    harness.wait_for_helper_shadow_customers(
        "1:alice+network@example.com,2:bob+recovered@example.com",
    );
    harness
        .wait_for_destination_customers("1:alice+network@example.com,2:bob+recovered@example.com");
    harness.assert_helper_shadow_customers_stable(
        "1:alice+network@example.com,2:bob+recovered@example.com",
        Duration::from_secs(3),
    );
    harness.assert_destination_customers_stable(
        "1:alice+network@example.com,2:bob+recovered@example.com",
        Duration::from_secs(3),
    );
}

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_recovers_after_helper_persistence_transaction_failure() {
    let harness = DefaultBootstrapHarness::start_with_observed_webhook_gateway();

    harness.bootstrap_default_migration();
    harness.wait_for_destination_customers("1:alice@example.com,2:bob@example.com");
    let helper_failure =
        harness.fail_helper_shadow_customer_email_write(1, "alice+helper-failure@example.com");
    harness.update_source_customer_email(1, "alice+helper-failure@example.com");
    harness.wait_for_duplicate_customer_delivery("alice+helper-failure@example.com");
    harness.assert_helper_shadow_customers_stable(
        "1:alice@example.com,2:bob@example.com",
        Duration::from_secs(3),
    );
    harness.assert_destination_customers_stable(
        "1:alice@example.com,2:bob@example.com",
        Duration::from_secs(3),
    );
    drop(helper_failure);
    harness.wait_for_gateway_downstream_500_then_200_for_customer_email(
        "alice+helper-failure@example.com",
    );
    harness
        .wait_for_helper_shadow_customers("1:alice+helper-failure@example.com,2:bob@example.com");
    harness.wait_for_destination_customers("1:alice+helper-failure@example.com,2:bob@example.com");
}

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_recovers_after_reconcile_transaction_failure() {
    let harness = DefaultBootstrapHarness::start();

    harness.bootstrap_default_migration();
    harness.wait_for_destination_customers("1:alice@example.com,2:bob@example.com");
    let baseline = harness.customer_tracking_progress();
    let destination_failure =
        harness.fail_destination_customer_email_write(1, "alice+reconcile-failure@example.com");
    harness.update_source_customer_email(1, "alice+reconcile-failure@example.com");
    let stderr = harness.wait_for_runner_failed_exit();
    assert!(
        stderr.contains("failed to apply reconcile upsert"),
        "runner stderr did not include reconcile failure context:\n{stderr}"
    );
    harness
        .assert_helper_shadow_customers_snapshot("1:alice+reconcile-failure@example.com,2:bob@example.com");
    harness.assert_destination_customers_snapshot("1:alice@example.com,2:bob@example.com");
    let failed = harness.customer_tracking_progress();
    assert_eq!(
        failed.stream.latest_reconciled_resolved_watermark,
        baseline.stream.latest_reconciled_resolved_watermark,
        "reconciled watermark should stay at the last good checkpoint during failure",
    );
    assert_eq!(
        failed.table.last_successful_sync_watermark, baseline.table.last_successful_sync_watermark,
        "table sync watermark should stay at the last good checkpoint during failure",
    );
    let last_error = failed
        .table
        .last_error
        .as_deref()
        .expect("failed reconcile should persist last_error");
    assert!(
        last_error.contains("reconcile upsert failed for public.customers"),
        "table sync error should persist the reconcile failure context: {last_error}",
    );
    drop(destination_failure);
    harness.restart_runner();
    harness
        .wait_for_destination_customers("1:alice+reconcile-failure@example.com,2:bob@example.com");
    let expected_reconciled_watermark = failed
        .stream
        .latest_received_resolved_watermark
        .clone()
        .expect("failed reconcile should persist a received watermark");
    let recovered = harness.wait_for_customer_tracking_progress(
        "restarted runner should reconcile the failed watermark and clear the stored error",
        |progress| {
            progress.has_reconciled_through(expected_reconciled_watermark.as_str())
                && progress.table.last_error.is_none()
        },
    );
    assert!(
        recovered
            .stream
            .has_received_through(expected_reconciled_watermark.as_str()),
        "received watermark should remain monotonic after recovery",
    );
}

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_recovers_after_runner_crash_once_helper_state_is_persisted_before_reconcile() {
    let harness = DefaultBootstrapHarness::start_with_reconcile_interval(30);

    harness.bootstrap_default_migration();
    harness.wait_for_destination_customers("1:alice@example.com,2:bob@example.com");
    let baseline = harness.customer_tracking_progress();
    assert_eq!(
        baseline.stream.latest_received_resolved_watermark,
        baseline.stream.latest_reconciled_resolved_watermark,
        "bootstrap should begin from a fully reconciled watermark",
    );
    assert_eq!(
        baseline.table.last_successful_sync_watermark,
        baseline.stream.latest_reconciled_resolved_watermark,
        "table sync watermark should match the reconciled stream watermark after bootstrap",
    );
    assert_eq!(
        baseline.table.last_error, None,
        "customer sync progress should start without stored errors",
    );
    harness.update_source_customer_email(1, "alice+restart@example.com");
    harness.wait_for_helper_shadow_customers("1:alice+restart@example.com,2:bob@example.com");
    harness.assert_destination_customers_stable(
        "1:alice@example.com,2:bob@example.com",
        Duration::from_secs(3),
    );
    let pre_crash = harness.wait_for_customer_tracking_progress(
        "customer update should be durably received before reconcile catches up",
        |progress| {
            progress
                .stream
                .received_has_advanced_since(&baseline.stream)
                && progress.stream.latest_reconciled_resolved_watermark
                    == baseline.stream.latest_reconciled_resolved_watermark
                && progress.table.last_successful_sync_watermark
                    == baseline.table.last_successful_sync_watermark
                && progress.table.last_error.is_none()
        },
    );
    harness.kill_runner();
    harness.restart_runner();
    harness.wait_for_destination_customers("1:alice+restart@example.com,2:bob@example.com");
    harness.assert_destination_customers_stable(
        "1:alice+restart@example.com,2:bob@example.com",
        Duration::from_secs(3),
    );
    let expected_reconciled_watermark = pre_crash
        .stream
        .latest_received_resolved_watermark
        .clone()
        .expect("pre-crash received watermark should exist");
    let recovered = harness.wait_for_customer_tracking_progress(
        "restarted runner should reconcile the received watermark",
        |progress| {
            progress.has_reconciled_through(expected_reconciled_watermark.as_str())
                && progress.table.last_error.is_none()
        },
    );
    assert!(
        recovered
            .stream
            .has_received_through(expected_reconciled_watermark.as_str()),
        "received watermark should stay monotonic across restart",
    );
}

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_recovers_after_runner_crash_during_a_blocked_reconcile_pass() {
    let harness = DefaultBootstrapHarness::start();

    harness.bootstrap_default_migration();
    harness.wait_for_destination_customers("1:alice@example.com,2:bob@example.com");
    let baseline = harness.customer_tracking_progress();
    let destination_lock = harness.lock_destination_customers();
    harness.update_source_customer_email(1, "alice+locked@example.com");
    harness.wait_for_helper_shadow_customers("1:alice+locked@example.com,2:bob@example.com");
    harness.wait_for_customer_reconcile_block();
    let pre_crash = harness.wait_for_customer_tracking_progress(
        "blocked reconcile should leave the new watermark received but not reconciled",
        |progress| {
            progress
                .stream
                .received_has_advanced_since(&baseline.stream)
                && progress.stream.latest_reconciled_resolved_watermark
                    == baseline.stream.latest_reconciled_resolved_watermark
                && progress.table.last_successful_sync_watermark
                    == baseline.table.last_successful_sync_watermark
                && progress.table.last_error.is_none()
        },
    );
    harness.kill_runner();
    drop(destination_lock);
    harness.restart_runner();
    harness.wait_for_destination_customers("1:alice+locked@example.com,2:bob@example.com");
    harness.assert_destination_customers_stable(
        "1:alice+locked@example.com,2:bob@example.com",
        Duration::from_secs(3),
    );
    let expected_reconciled_watermark = pre_crash
        .stream
        .latest_received_resolved_watermark
        .clone()
        .expect("pre-crash received watermark should exist");
    let recovered = harness.wait_for_customer_tracking_progress(
        "restarted runner should reconcile through the blocked watermark",
        |progress| {
            progress.has_reconciled_through(expected_reconciled_watermark.as_str())
                && progress.table.last_error.is_none()
        },
    );
    assert!(
        recovered
            .stream
            .has_received_through(expected_reconciled_watermark.as_str()),
        "received watermark should remain monotonic after the blocked reconcile restart",
    );
}

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_handles_fk_heavy_initial_scan_and_live_catchup_into_real_postgres_tables() {
    let harness = CdcE2eHarness::start(CdcE2eHarnessConfig {
        mapping_id: "app-a",
        source_database: "demo_a",
        destination_database: "app_a",
        destination_user: "migration_user_a",
        destination_password: "runner-secret-a",
        reconcile_interval_secs: 1,
        selected_tables: &["public.parents", "public.children", "public.grandchildren"],
        source_setup_sql: FK_HEAVY_SOURCE_SETUP_SQL,
        destination_setup_sql: FK_HEAVY_DESTINATION_SETUP_SQL,
    });

    harness.bootstrap_migration();
    harness.wait_for_helper_table_row_counts(&[
        ("public.parents", 2),
        ("public.children", 2),
        ("public.grandchildren", 2),
    ]);
    harness.wait_for_destination_query(
        FK_HEAVY_SNAPSHOT_SQL,
        "c:10:1:alpha child,c:20:2:beta child,g:100:10:alpha grandchild,g:200:20:beta grandchild,p:1:alpha parent,p:2:beta parent",
        "initial FK-heavy snapshot",
    );
    assert_eq!(
        harness.destination_constraint_snapshot().trim(),
        FK_HEAVY_CONSTRAINTS,
        "destination real tables should retain PK/FK constraints during initial scan",
    );
    harness.assert_explicit_source_bootstrap_commands();
    apply_fk_heavy_live_source_changes(&harness);
    harness.wait_for_destination_query(
        FK_HEAVY_SNAPSHOT_SQL,
        "c:10:1:alpha child,c:30:3:gamma child,g:100:10:alpha grandchild,g:300:30:gamma grandchild,p:1:alpha parent updated,p:3:gamma parent",
        "live FK-heavy catch-up snapshot",
    );
    assert_eq!(
        harness.destination_constraint_snapshot().trim(),
        FK_HEAVY_CONSTRAINTS,
        "destination real tables should retain PK/FK constraints after live catch-up",
    );

    let converged_snapshot = harness.query_destination(FK_HEAVY_SNAPSHOT_SQL);
    thread::sleep(Duration::from_secs(3));
    harness.wait_for_destination_query(
        FK_HEAVY_SNAPSHOT_SQL,
        converged_snapshot.trim(),
        "stable FK-heavy snapshot after repeated reconcile",
    );
}

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_propagates_customer_deletes_from_shadow_tables_into_real_postgres_tables() {
    let harness = DefaultBootstrapHarness::start_with_reconcile_interval(5);

    harness.bootstrap_default_migration();
    harness.wait_for_destination_customers("1:alice@example.com,2:bob@example.com");
    harness.delete_source_customer(1);
    harness.wait_for_helper_shadow_customers("2:bob@example.com");
    harness.assert_destination_customer_count(1, 1);
    harness.wait_for_destination_customers("2:bob@example.com");
    harness.assert_helper_shadow_customers_stable("2:bob@example.com", Duration::from_secs(11));
    harness.assert_destination_customers_stable("2:bob@example.com", Duration::from_secs(11));
}

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_converges_after_high_source_customer_write_churn_during_transfer() {
    let harness = DefaultBootstrapHarness::start();

    harness.bootstrap_default_migration();
    harness.wait_for_destination_customers("1:alice@example.com,2:bob@example.com");
    let baseline = harness.customer_tracking_progress();

    let expectation = harness.run_high_source_customer_write_churn_workload();

    harness.wait_for_helper_shadow_customers(expectation.final_customers_snapshot());
    harness.wait_for_destination_customers(expectation.final_customers_snapshot());
    let received = harness.wait_for_customer_tracking_progress(
        "customer tracking should record source churn beyond the bootstrap watermark",
        |progress| {
            progress
                .stream
                .received_has_advanced_since(&baseline.stream)
        },
    );
    let received_watermark = received
        .stream
        .latest_received_resolved_watermark
        .clone()
        .expect("churn workload should produce a received watermark");
    let converged = harness.wait_for_customer_tracking_progress(
        "customer tracking should reconcile through the churn watermark without storing an error",
        |progress| {
            progress.has_reconciled_through(received_watermark.as_str())
                && progress.table.last_error.is_none()
        },
    );
    assert!(
        converged
            .stream
            .has_received_through(received_watermark.as_str()),
        "received watermark should remain monotonic after churn catch-up",
    );
    harness.assert_helper_shadow_customers_stable(
        expectation.final_customers_snapshot(),
        Duration::from_secs(3),
    );
    harness.assert_destination_customers_stable(
        expectation.final_customers_snapshot(),
        Duration::from_secs(3),
    );
    harness.assert_runner_alive();
}

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_handles_composite_primary_keys_while_skipping_unselected_tables() {
    let harness = CompositePkExclusionHarness::start();

    harness.bootstrap_migration();
    harness.wait_for_initial_scan();
    harness.assert_explicit_source_bootstrap_commands();
    harness.apply_live_source_changes();
    harness.wait_for_live_catchup();
    harness.assert_included_tables_stable(Duration::from_secs(3));
}

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_runs_multiple_large_multi_database_migrations_under_one_destination_container()
{
    let harness = MultiMappingHarness::start();

    harness.bootstrap_migration();
    harness.wait_for_initial_scan();
    harness.assert_explicit_source_bootstrap_commands();
    harness.assert_helper_state_is_mapping_scoped();
    harness.apply_live_source_changes();
    harness.wait_for_live_catchup();
    harness.assert_mapping_state_stable(Duration::from_secs(3));
}
