use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub struct VerifySourceContract {
    molt_root: PathBuf,
    root_command_text: String,
    go_mod_text: String,
    go_sum_text: String,
}

impl VerifySourceContract {
    pub fn load() -> Self {
        let molt_root = repo_root().join("cockroachdb_molt/molt");
        let root_command_path = molt_root.join("cmd/root.go");
        let go_mod_path = molt_root.join("go.mod");
        let go_sum_path = molt_root.join("go.sum");
        let root_command_text = fs::read_to_string(&root_command_path).unwrap_or_else(|error| {
            panic!(
                "verify root command `{}` should be readable: {error}",
                root_command_path.display()
            )
        });
        let go_mod_text = fs::read_to_string(&go_mod_path).unwrap_or_else(|error| {
            panic!("go.mod `{}` should be readable: {error}", go_mod_path.display())
        });
        let go_sum_text = fs::read_to_string(&go_sum_path).unwrap_or_else(|error| {
            panic!("go.sum `{}` should be readable: {error}", go_sum_path.display())
        });

        Self {
            molt_root,
            root_command_text,
            go_mod_text,
            go_sum_text,
        }
    }

    pub fn assert_top_level_entries_are_within_the_verify_slice(&self) {
        let actual_entries = read_dir_names(&self.molt_root);
        let allowed_entries = verify_slice_top_level_entries();
        let unexpected_entries = actual_entries
            .difference(&allowed_entries)
            .cloned()
            .collect::<Vec<_>>();

        assert!(
            unexpected_entries.is_empty(),
            "vendored MOLT tree should keep only verify-slice top-level entries, found unexpected entries: {unexpected_entries:?}",
        );
    }

    pub fn assert_root_command_does_not_wire_fetch(&self) {
        for forbidden_marker in [
            "\"github.com/cockroachdb/molt/cmd/fetch\"",
            "fetch.Command()",
        ] {
            assert!(
                !self.root_command_text.contains(forbidden_marker),
                "root command must not retain fetch wiring marker `{forbidden_marker}`",
            );
        }
    }

    pub fn assert_cmd_tree_is_verify_only(&self) {
        let actual_entries = read_dir_names(&self.molt_root.join("cmd"));
        let allowed_entries = BTreeSet::from([
            String::from("apiversion.go"),
            String::from("internal"),
            String::from("root.go"),
            String::from("verify"),
        ]);
        let unexpected_entries = actual_entries
            .difference(&allowed_entries)
            .cloned()
            .collect::<Vec<_>>();

        assert!(
            unexpected_entries.is_empty(),
            "vendored MOLT cmd tree should keep only the verify command surface, found unexpected entries: {unexpected_entries:?}",
        );
        assert!(
            !self.root_command_text.contains("EscapePasswordCommand()"),
            "root command must not expose the escape-password utility",
        );
    }

    pub fn assert_fetch_only_files_are_absent(&self) {
        for path in [
            "cmd/escape_password.go",
            "cmd/fetch",
            "cmd/internal/cmdutil/pprof.go",
        ] {
            let full_path = self.molt_root.join(path);
            assert!(
                !full_path.exists(),
                "verify-only vendored tree must not retain fetch-only path `{}`",
                full_path.display(),
            );
        }
    }

    pub fn assert_fetch_only_dependency_families_are_absent(&self) {
        for forbidden_module in [
            "cloud.google.com/go/storage",
            "github.com/aws/aws-sdk-go",
            "golang.org/x/oauth2",
            "google.golang.org/api",
        ] {
            assert!(
                !self.go_mod_text.contains(forbidden_module),
                "verify-only go.mod must not retain fetch-only dependency `{forbidden_module}`",
            );
            assert!(
                !self.go_sum_text.contains(forbidden_module),
                "verify-only go.sum must not retain fetch-only dependency `{forbidden_module}`",
            );
        }
    }

    pub fn assert_testutils_exception_is_narrow_and_explicit(&self) {
        let actual_entries = read_dir_names(&self.molt_root.join("testutils"));
        let allowed_entries = BTreeSet::from([String::from("conn.go")]);
        let unexpected_entries = actual_entries
            .difference(&allowed_entries)
            .cloned()
            .collect::<Vec<_>>();

        assert!(
            unexpected_entries.is_empty(),
            "verify-only vendored tree should keep only the explicit retained testutils exception, found unexpected testutils files: {unexpected_entries:?}",
        );
    }
}

fn verify_slice_top_level_entries() -> BTreeSet<String> {
    [
        "LICENSE",
        "cmd",
        "comparectx",
        "dbconn",
        "dbtable",
        "go.mod",
        "go.sum",
        "main.go",
        "moltlogger",
        "molttelemetry",
        "mysqlconv",
        "mysqlurl",
        "oracleconv",
        "parsectx",
        "pgconv",
        "retry",
        "rowiterator",
        "testutils",
        "utils",
        "verify",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn read_dir_names(dir: &Path) -> BTreeSet<String> {
    fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("directory `{}` should be readable: {error}", dir.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| {
                    panic!("directory entry under `{}` should be readable: {error}", dir.display())
                })
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}
