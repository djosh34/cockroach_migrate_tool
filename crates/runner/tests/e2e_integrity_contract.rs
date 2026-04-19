use std::{fs, path::PathBuf};

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

fn collect_test_files(dir: &std::path::Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|error| {
        panic!(
            "runner tests dir `{}` should be readable: {error}",
            dir.display()
        )
    }) {
        let entry = entry.expect("runner tests dir entry should load");
        let path = entry.path();
        if entry
            .file_type()
            .expect("runner tests dir entry type should load")
            .is_dir()
        {
            collect_test_files(&path, files);
        } else {
            files.push(path);
        }
    }
}

fn scoped_integrity_files() -> [(&'static str, String); 9] {
    [
        (
            "tests/default_bootstrap_long_lane.rs",
            read_runner_test_file("tests/default_bootstrap_long_lane.rs"),
        ),
        (
            "tests/support/default_bootstrap_harness.rs",
            read_runner_test_file("tests/support/default_bootstrap_harness.rs"),
        ),
        (
            "tests/support/e2e_harness.rs",
            read_runner_test_file("tests/support/e2e_harness.rs"),
        ),
        (
            "tests/support/composite_pk_exclusion_harness.rs",
            read_runner_test_file("tests/support/composite_pk_exclusion_harness.rs"),
        ),
        (
            "tests/support/multi_mapping_harness.rs",
            read_runner_test_file("tests/support/multi_mapping_harness.rs"),
        ),
        (
            "src/webhook_runtime/mod.rs",
            read_runner_test_file("src/webhook_runtime/mod.rs"),
        ),
        (
            "src/webhook_runtime/persistence.rs",
            read_runner_test_file("src/webhook_runtime/persistence.rs"),
        ),
        (
            "src/reconcile_runtime/mod.rs",
            read_runner_test_file("src/reconcile_runtime/mod.rs"),
        ),
        (
            "src/reconcile_runtime/upsert.rs",
            read_runner_test_file("src/reconcile_runtime/upsert.rs"),
        ),
    ]
}

#[test]
fn e2e_suite_no_longer_routes_integrity_through_runner_verify() {
    let long_lane = read_runner_test_file("tests/default_bootstrap_long_lane.rs");
    let default_harness = read_runner_test_file("tests/support/default_bootstrap_harness.rs");
    let composite_harness =
        read_runner_test_file("tests/support/composite_pk_exclusion_harness.rs");
    let multi_mapping_harness = read_runner_test_file("tests/support/multi_mapping_harness.rs");
    let e2e_harness = read_runner_test_file("tests/support/e2e_harness.rs");

    assert!(
        !default_harness.contains("verify_default_migration"),
        "default bootstrap harness should not expose removed runner verify helpers",
    );
    assert!(
        !long_lane.contains("verify_migration"),
        "long-lane scenarios should not call removed runner verify helpers",
    );
    assert!(
        !composite_harness.contains("verify_migration"),
        "composite-key harness should not expose removed runner verify helpers",
    );
    assert!(
        !multi_mapping_harness.contains("verify_migration"),
        "multi-mapping harness should not expose removed runner verify helpers",
    );
    assert!(
        !e2e_harness.contains("runner verify"),
        "shared E2E harness should not shell out to the removed runner verify command",
    );
    assert!(
        !e2e_harness.contains("--source-url"),
        "shared E2E harness should not depend on removed source-url flags",
    );
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
    let banned_markers = [
        "--fake",
        "--skip-webhook",
        "--skip-reconcile",
        "--skip-verify",
        "--bypass",
        "FAKE_MIGRATION",
        "SKIP_WEBHOOK",
        "SKIP_RECONCILE",
        "BYPASS_VERIFY",
        "cheat",
    ];

    for (path, contents) in scoped_integrity_files() {
        for marker in banned_markers {
            assert!(
                !contents.contains(marker),
                "integrity-scoped file `{path}` should not contain shortcut marker `{marker}`",
            );
        }
    }
}

#[test]
fn only_approved_support_files_issue_raw_docker_commands_for_e2e_orchestration() {
    let approved = [
        "tests/support/e2e_harness.rs",
        "tests/support/runner_container_process.rs",
        "tests/support/runner_image_harness.rs",
    ];
    let tests_dir = repo_root().join("crates/runner/tests");
    let runner_root = repo_root().join("crates/runner");
    let mut docker_call_sites = Vec::new();
    let mut test_files = Vec::new();

    collect_test_files(&tests_dir, &mut test_files);

    for path in test_files {
        let path = path
            .strip_prefix(&runner_root)
            .expect("runner-relative test path should strip")
            .to_string_lossy()
            .replace('\\', "/");
        let contents = fs::read_to_string(runner_root.join(&path)).unwrap_or_else(|error| {
            panic!("runner test file `{path}` should be readable: {error}")
        });
        if contents.contains("Command::new(\"docker\")") {
            docker_call_sites.push(path);
        }
    }

    docker_call_sites.sort();
    assert_eq!(
        docker_call_sites, approved,
        "raw Docker orchestration should stay isolated to the approved E2E support files",
    );
}
