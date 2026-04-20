use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub struct ReadmePublicImageContract {
    operator_files: BTreeMap<&'static str, String>,
}

impl ReadmePublicImageContract {
    pub fn load() -> Self {
        let readme_path = repo_root().join("README.md");
        let readme_text = fs::read_to_string(&readme_path).unwrap_or_else(|error| {
            panic!(
                "README should be readable at `{}`: {error}",
                readme_path.display(),
            )
        });
        let novice_surface = novice_surface(&readme_text);

        assert_registry_only_surface(&novice_surface);

        let mut operator_files = BTreeMap::new();
        for relative_path in [
            "config/cockroach-setup.yml",
            "config/postgres-grants.yml",
            "config/runner.yml",
            "config/verify-service.yml",
        ] {
            operator_files.insert(
                relative_path,
                extract_inline_config(&novice_surface, relative_path),
            );
        }
        for relative_path in [
            "setup-sql.compose.yml",
            "runner.compose.yml",
            "verify.compose.yml",
        ] {
            operator_files.insert(
                relative_path,
                extract_named_yaml_block(&novice_surface, relative_path),
            );
        }

        Self { operator_files }
    }

    pub fn materialize_operator_workspace(&self, root_dir: &Path) {
        for (relative_path, contents) in &self.operator_files {
            let path = root_dir.join(relative_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap_or_else(|error| {
                    panic!(
                        "README operator workspace should create parent `{}`: {error}",
                        parent.display(),
                    )
                });
            }
            fs::write(&path, contents).unwrap_or_else(|error| {
                panic!(
                    "README operator workspace should write `{}`: {error}",
                    path.display(),
                )
            });
        }
    }

    pub fn operator_file(&self, relative_path: &str) -> &str {
        self.operator_files.get(relative_path).unwrap_or_else(|| {
            panic!("README operator workspace should define `{relative_path}`")
        })
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}

fn novice_surface(readme_text: &str) -> String {
    readme_text
        .split("## Setup SQL Quick Start")
        .nth(1)
        .and_then(|remainder| remainder.split("## CI Publish Safety").next())
        .map(str::to_owned)
        .expect("README must keep the novice-user quick start surface grouped together")
}

fn assert_registry_only_surface(novice_surface: &str) {
    for forbidden in [
        "git clone",
        "docker build",
        "cargo ",
        "cargo\n",
        "make ",
        "make\n",
        "AGENTS.md",
        "CONTRIBUTING.md",
        "crates/",
        "tests/",
        "investigations/",
    ] {
        assert!(
            !novice_surface.contains(forbidden),
            "README public-image novice surface must stay repo-free; found forbidden snippet `{forbidden}`",
        );
    }
}

fn extract_inline_config(novice_surface: &str, relative_path: &str) -> String {
    let prefix = format!("```yaml\n# {relative_path}\n");
    extract_fenced_block_after_prefix(novice_surface, &prefix)
}

fn extract_named_yaml_block(novice_surface: &str, relative_path: &str) -> String {
    let marker = format!("Save this as `{relative_path}`:");
    let start = novice_surface.find(&marker).unwrap_or_else(|| {
        panic!("README novice surface should declare `{relative_path}` inline")
    });
    let remainder = &novice_surface[start + marker.len()..];
    extract_fenced_block_after_prefix(remainder, "```yaml\n")
}

fn extract_fenced_block_after_prefix(haystack: &str, prefix: &str) -> String {
    let start = haystack.find(prefix).unwrap_or_else(|| {
        panic!("README novice surface should contain fenced block prefix `{prefix}`")
    });
    let remainder = &haystack[start + "```yaml\n".len()..];
    let end = remainder.find("\n```").expect("README fenced block should close");
    remainder[..end].to_owned()
}
