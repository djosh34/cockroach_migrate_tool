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
    for entry in fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("runner tests dir `{}` should be readable: {error}", dir.display()))
    {
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

fn scoped_integrity_files() -> [(&'static str, String); 10] {
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
        (
            "src/molt_verify/mod.rs",
            read_runner_test_file("src/molt_verify/mod.rs"),
        ),
    ]
}

#[test]
fn e2e_suite_routes_verify_assertions_through_a_typed_integrity_boundary() {
    let long_lane = read_runner_test_file("tests/default_bootstrap_long_lane.rs");
    let default_harness = read_runner_test_file("tests/support/default_bootstrap_harness.rs");
    let composite_harness =
        read_runner_test_file("tests/support/composite_pk_exclusion_harness.rs");
    let multi_mapping_harness = read_runner_test_file("tests/support/multi_mapping_harness.rs");

    assert!(
        repo_root()
            .join("crates/runner/tests/support/e2e_integrity.rs")
            .is_file(),
        "E2E integrity evidence should live behind a dedicated typed support boundary",
    );
    assert!(
        !default_harness.contains("verify_default_migration_output"),
        "default bootstrap harness should not expose raw verify output as a public test API",
    );
    assert!(
        !long_lane.contains("verify_output.contains("),
        "long-lane scenarios should assert verify behavior through typed integrity evidence, not raw substring checks",
    );
    assert!(
        !composite_harness.contains("let output = self.inner.verify_migration();"),
        "composite-key harness should assert verify behavior through typed integrity evidence",
    );
    assert!(
        !multi_mapping_harness.contains("app_a_output.contains("),
        "multi-mapping harness should not inspect verify output through raw substring checks",
    );
    assert!(
        !multi_mapping_harness.contains("app_b_output.contains("),
        "multi-mapping harness should not inspect verify output through raw substring checks",
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
        let contents = fs::read_to_string(runner_root.join(&path))
            .unwrap_or_else(|error| panic!("runner test file `{path}` should be readable: {error}"));
        if contents.contains("Command::new(\"docker\")") {
            docker_call_sites.push(path);
        }
    }

    docker_call_sites.sort();
    assert_eq!(
        docker_call_sites,
        approved,
        "raw Docker orchestration should stay isolated to the approved E2E support files",
    );
}
