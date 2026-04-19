use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub struct VerifySourceContract {
    molt_root: PathBuf,
    root_command_text: String,
    go_mod_text: String,
}

impl VerifySourceContract {
    pub fn load() -> Self {
        let molt_root = repo_root().join("cockroachdb_molt/molt");
        let root_command_path = molt_root.join("cmd/root.go");
        let go_mod_path = molt_root.join("go.mod");
        let root_command_text = fs::read_to_string(&root_command_path).unwrap_or_else(|error| {
            panic!(
                "verify root command `{}` should be readable: {error}",
                root_command_path.display()
            )
        });
        let go_mod_text = fs::read_to_string(&go_mod_path).unwrap_or_else(|error| {
            panic!("go.mod `{}` should be readable: {error}", go_mod_path.display())
        });

        Self {
            molt_root,
            root_command_text,
            go_mod_text,
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
            self.assert_retained_source_does_not_import(forbidden_module);
            self.assert_direct_requirement_is_absent(forbidden_module);
        }
    }

    pub fn assert_retained_source_does_not_import(&self, forbidden_module: &str) {
        let import_sites = self.go_source_import_sites(forbidden_module);

        assert!(
            import_sites.is_empty(),
            "verify-only retained source must not import `{forbidden_module}`, found imports in: {import_sites:?}",
        );
    }

    pub fn assert_module_declares_go_version(&self, expected_version: &str) {
        let declared_version = self
            .go_mod_text
            .lines()
            .find_map(|line| line.strip_prefix("go ").map(str::trim))
            .unwrap_or_else(|| panic!("verify-only go.mod should declare a Go version"));

        assert_eq!(
            declared_version, expected_version,
            "verify-only go.mod should declare Go {expected_version}, found Go {declared_version}",
        );
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

    fn assert_direct_requirement_is_absent(&self, forbidden_module: &str) {
        let direct_requirements = self.direct_requirements();

        assert!(
            !direct_requirements.contains(forbidden_module),
            "verify-only go.mod must not retain direct dependency `{forbidden_module}`",
        );
    }

    fn direct_requirements(&self) -> BTreeSet<String> {
        let mut direct_requirements = BTreeSet::new();
        let mut in_require_block = false;

        for raw_line in self.go_mod_text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            if line == "require (" {
                in_require_block = true;
                continue;
            }
            if in_require_block && line == ")" {
                in_require_block = false;
                continue;
            }
            if in_require_block {
                if let Some(requirement) = parse_direct_requirement_line(line) {
                    direct_requirements.insert(requirement);
                }
                continue;
            }
            if let Some(requirement) = line
                .strip_prefix("require ")
                .and_then(parse_direct_requirement_line)
            {
                direct_requirements.insert(requirement);
            }
        }

        direct_requirements
    }

    fn go_source_import_sites(&self, forbidden_module: &str) -> Vec<String> {
        let mut import_sites = Vec::new();

        for source_file in go_source_files(&self.molt_root) {
            let source_text = fs::read_to_string(&source_file).unwrap_or_else(|error| {
                panic!(
                    "verify source file `{}` should be readable: {error}",
                    source_file.display()
                )
            });
            if go_imports(&source_text)
                .iter()
                .any(|import_path| import_path == forbidden_module)
            {
                import_sites.push(
                    source_file
                        .strip_prefix(&self.molt_root)
                        .unwrap_or_else(|error| {
                            panic!(
                                "source file `{}` should stay under verify root `{}`: {error}",
                                source_file.display(),
                                self.molt_root.display()
                            )
                        })
                        .display()
                        .to_string(),
                );
            }
        }

        import_sites
    }
}

fn verify_slice_top_level_entries() -> BTreeSet<String> {
    [
        "Dockerfile",
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

fn parse_direct_requirement_line(line: &str) -> Option<String> {
    if line.contains("// indirect") {
        return None;
    }

    line.split_whitespace().next().map(str::to_owned)
}

fn go_source_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for entry in fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("directory `{}` should be readable: {error}", dir.display()))
    {
        let path = entry
            .unwrap_or_else(|error| {
                panic!("directory entry under `{}` should be readable: {error}", dir.display())
            })
            .path();
        if path.is_dir() {
            files.extend(go_source_files(&path));
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) == Some("go") {
            files.push(path);
        }
    }

    files.sort();
    files
}

fn go_imports(source_text: &str) -> Vec<String> {
    let mut imports = Vec::new();
    let mut in_import_block = false;

    for raw_line in source_text.lines() {
        let line = raw_line.trim();
        if in_import_block {
            if line == ")" {
                in_import_block = false;
                continue;
            }
            if let Some(import_path) = extract_go_import_path(line) {
                imports.push(import_path);
            }
            continue;
        }
        if let Some(import_clause) = line.strip_prefix("import ") {
            if import_clause == "(" {
                in_import_block = true;
                continue;
            }
            if let Some(import_path) = extract_go_import_path(import_clause) {
                imports.push(import_path);
            }
        }
    }

    imports
}

fn extract_go_import_path(import_clause: &str) -> Option<String> {
    let first_quote = import_clause.find('"')?;
    let remainder = &import_clause[first_quote + 1..];
    let second_quote = remainder.find('"')?;

    Some(remainder[..second_quote].to_owned())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}
