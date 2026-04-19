#[path = "support/default_bootstrap_harness.rs"]
mod default_bootstrap_harness;
#[path = "support/e2e_harness.rs"]
mod e2e_harness;

use std::{thread, time::Duration};

use default_bootstrap_harness::DefaultBootstrapHarness;
use e2e_harness::{CdcE2eHarness, CdcE2eHarnessConfig};

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
    harness.execute_source_sql(
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

    harness.verify_migration();
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
    let verify_output = harness.verify_default_migration_output();
    assert!(
        !verify_output.contains("_cockroach_migration_tool"),
        "verify output should mention only the real migrated table: {verify_output}"
    );
}
