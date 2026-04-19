use std::{fs, path::PathBuf};

#[path = "support/e2e_integrity_contract_support.rs"]
mod e2e_integrity_contract_support;

use e2e_integrity_contract_support::E2eIntegrityContractAudit;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}

fn read_runner_test_file(path: &str) -> String {
    fs::read_to_string(repo_root().join("crates/runner").join(path))
        .unwrap_or_else(|error| panic!("runner test file `{path}` should be readable: {error}"))
}

#[test]
fn e2e_suite_no_longer_routes_integrity_through_runner_verify() {
    E2eIntegrityContractAudit::load().assert_no_runner_verify_surface();
}

#[test]
fn e2e_suite_routes_runtime_shape_assertions_through_a_typed_integrity_boundary() {
    let long_lane = read_runner_test_file("tests/default_bootstrap_long_lane.rs");
    let default_harness = read_runner_test_file("tests/support/default_bootstrap_harness.rs");
    let integrity = read_runner_test_file("tests/support/e2e_integrity.rs");

    assert!(
        integrity.contains("pub struct RuntimeShapeAudit"),
        "E2E integrity support should define a dedicated typed runtime-shape audit",
    );
    assert!(
        default_harness.contains("pub fn runtime_shape_audit(&self) -> RuntimeShapeAudit"),
        "default bootstrap harness should expose runtime-shape assertions through a typed public API",
    );
    assert!(
        long_lane.contains("assert_honest_default_runtime_shape"),
        "the honest default long-lane scenario should assert runtime shape through the typed integrity boundary",
    );
}

#[test]
fn e2e_suite_routes_post_setup_source_commands_through_a_typed_integrity_boundary() {
    let long_lane = read_runner_test_file("tests/default_bootstrap_long_lane.rs");
    let default_harness = read_runner_test_file("tests/support/default_bootstrap_harness.rs");
    let e2e_harness = read_runner_test_file("tests/support/e2e_harness.rs");
    let multi_mapping_harness = read_runner_test_file("tests/support/multi_mapping_harness.rs");
    let integrity = read_runner_test_file("tests/support/e2e_integrity.rs");

    assert!(
        integrity.contains("pub struct SourceCommandAudit"),
        "E2E integrity support should define a typed source-command audit",
    );
    assert!(
        integrity.contains("pub enum SourceCommandPhase"),
        "E2E integrity support should classify source commands by phase",
    );
    assert!(
        integrity.contains("pub struct PostSetupSourceAudit"),
        "E2E integrity support should expose a dedicated post-setup source audit",
    );
    assert!(
        default_harness.contains("pub fn post_setup_source_audit(&self) -> PostSetupSourceAudit"),
        "default bootstrap harness should expose post-setup source evidence through a typed public API",
    );
    assert!(
        !e2e_harness.contains("pub fn execute_source_sql(&self, sql: &str)"),
        "the shared E2E harness should not expose a broad public raw source SQL escape hatch",
    );
    assert!(
        !multi_mapping_harness.contains("fn execute_source_sql(&self, database: &str, sql: &str)"),
        "multi-mapping support should not maintain a second raw source SQL path outside the typed audit owner",
    );
    assert!(
        !long_lane.contains("harness.execute_source_sql("),
        "long-lane scenarios should not issue raw source SQL directly; they should use named audited workload helpers",
    );
}

#[test]
fn e2e_support_applies_source_bootstrap_through_sql_not_shell_scripts() {
    let e2e_harness = read_runner_test_file("tests/support/e2e_harness.rs");
    let multi_mapping_harness = read_runner_test_file("tests/support/multi_mapping_harness.rs");

    for (path, contents) in [
        ("tests/support/e2e_harness.rs", e2e_harness),
        (
            "tests/support/multi_mapping_harness.rs",
            multi_mapping_harness,
        ),
    ] {
        for forbidden_marker in [
            "source_bootstrap_script_path",
            "render_source_bootstrap_script",
            "execute_bootstrap_script",
            "render-bootstrap-script",
            "bootstrap shell script",
            "Command::new(\"bash\")",
        ] {
            assert!(
                !contents.contains(forbidden_marker),
                "{path} should not retain source bootstrap shell-script surface `{forbidden_marker}`",
            );
        }
    }
}

#[test]
fn e2e_integrity_scopes_do_not_expose_fake_skip_or_bypass_migration_toggles() {
    E2eIntegrityContractAudit::load().assert_no_shortcut_toggles();
}

#[test]
fn only_approved_support_files_issue_raw_docker_commands_for_e2e_orchestration() {
    E2eIntegrityContractAudit::load().assert_only_approved_raw_docker_call_sites();
}
