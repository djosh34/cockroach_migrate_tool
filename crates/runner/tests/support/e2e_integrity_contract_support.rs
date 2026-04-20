use std::{fs, path::PathBuf};

pub struct E2eIntegrityContractAudit {
    long_lane: String,
    default_harness: String,
    composite_harness: String,
    multi_mapping_harness: String,
    e2e_harness: String,
}

impl E2eIntegrityContractAudit {
    pub fn load() -> Self {
        Self {
            long_lane: read_runner_test_file("tests/default_bootstrap_long_lane.rs"),
            default_harness: read_runner_test_file("tests/support/default_bootstrap_harness.rs"),
            composite_harness: read_runner_test_file(
                "tests/support/composite_pk_exclusion_harness.rs",
            ),
            multi_mapping_harness: read_runner_test_file("tests/support/multi_mapping_harness.rs"),
            e2e_harness: read_runner_test_file("tests/support/e2e_harness.rs"),
        }
    }

    pub fn assert_no_runner_verify_surface(&self) {
        assert!(
            !self.default_harness.contains("verify_default_migration"),
            "default bootstrap harness should not expose removed runner verify helpers",
        );
        assert!(
            !self.long_lane.contains("verify_migration"),
            "long-lane scenarios should not call removed runner verify helpers",
        );
        assert!(
            !self.composite_harness.contains("verify_migration"),
            "composite-key harness should not expose removed runner verify helpers",
        );
        assert!(
            !self.multi_mapping_harness.contains("verify_migration"),
            "multi-mapping harness should not expose removed runner verify helpers",
        );
        assert!(
            !self.e2e_harness.contains("runner verify"),
            "shared E2E harness should not shell out to the removed runner verify command",
        );
        assert!(
            !self.e2e_harness.contains("--source-url"),
            "shared E2E harness should not depend on removed source-url flags",
        );
    }

    pub fn assert_no_shortcut_toggles(&self) {
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

    pub fn assert_no_selected_table_correctness_shortcuts(&self) {
        assert!(
            !self
                .default_harness
                .contains("pub fn wait_for_destination_customers"),
            "default bootstrap harness should not expose a public direct destination correctness wait",
        );
        assert!(
            !self
                .default_harness
                .contains("pub fn assert_destination_customers_snapshot"),
            "default bootstrap harness should not expose a public destination correctness snapshot helper",
        );
        assert!(
            !self
                .default_harness
                .contains("pub fn assert_destination_customers_stable"),
            "default bootstrap harness should not expose a public destination correctness stability helper",
        );
        assert!(
            !self
                .composite_harness
                .contains("const CUSTOMERS_SNAPSHOT_SQL"),
            "composite-key harness should not keep a direct included-table destination correctness snapshot",
        );
        assert!(
            !self
                .composite_harness
                .contains("const ORDER_ITEMS_SNAPSHOT_SQL"),
            "composite-key harness should not keep a direct included-table order-item correctness snapshot",
        );
        assert!(
            !self
                .multi_mapping_harness
                .contains("const APP_A_REAL_SNAPSHOT_SQL"),
            "multi-mapping harness should not keep a direct app-a destination correctness snapshot",
        );
        assert!(
            !self
                .multi_mapping_harness
                .contains("const APP_B_REAL_SNAPSHOT_SQL"),
            "multi-mapping harness should not keep a direct app-b destination correctness snapshot",
        );
    }

    pub fn assert_only_approved_raw_docker_call_sites(&self) {
        let approved = [
            "tests/support/e2e_harness.rs",
            "tests/support/novice_registry_only_harness.rs",
            "tests/support/runner_container_process.rs",
            "tests/support/runner_image_artifact_harness.rs",
            "tests/support/runner_image_harness.rs",
            "tests/support/verify_image_artifact_harness.rs",
            "tests/support/verify_image_harness.rs",
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
            "raw Docker orchestration should stay isolated to the approved test support files",
        );
    }
}

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

fn read_runner_file(path: &str) -> String {
    fs::read_to_string(repo_root().join("crates/runner").join(path))
        .unwrap_or_else(|error| panic!("runner file `{path}` should be readable: {error}"))
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
            read_runner_file("src/webhook_runtime/mod.rs"),
        ),
        (
            "src/webhook_runtime/persistence.rs",
            read_runner_file("src/webhook_runtime/persistence.rs"),
        ),
        (
            "src/reconcile_runtime/mod.rs",
            read_runner_file("src/reconcile_runtime/mod.rs"),
        ),
        (
            "src/reconcile_runtime/upsert.rs",
            read_runner_file("src/reconcile_runtime/upsert.rs"),
        ),
    ]
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
