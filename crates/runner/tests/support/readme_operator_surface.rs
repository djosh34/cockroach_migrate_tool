use std::{fs, path::PathBuf};

#[path = "readme_operator_workspace.rs"]
mod readme_operator_workspace_support;

use readme_operator_workspace_support::ReadmeOperatorWorkspace;

pub struct ReadmeOperatorSurface {
    readme_text: String,
    workspace: ReadmeOperatorWorkspace,
}

impl ReadmeOperatorSurface {
    pub fn load() -> Self {
        let readme_path = repo_root().join("README.md");
        let readme_text = fs::read_to_string(&readme_path).unwrap_or_else(|error| {
            panic!(
                "README should be readable at `{}`: {error}",
                readme_path.display(),
            )
        });

        Self {
            readme_text,
            workspace: ReadmeOperatorWorkspace::load(),
        }
    }

    pub fn text(&self) -> &str {
        &self.readme_text
    }

    pub fn second_level_headings(&self) -> Vec<&str> {
        self.readme_text
            .lines()
            .filter(|line| line.starts_with("## "))
            .collect()
    }

    pub fn section(&self, heading: &str) -> &str {
        let start = self.readme_text.find(heading).unwrap_or_else(|| {
            panic!("README should contain heading `{heading}`");
        });
        let after_heading = &self.readme_text[start + heading.len()..];
        let end = after_heading
            .find("\n## ")
            .map(|index| start + heading.len() + index)
            .unwrap_or(self.readme_text.len());
        &self.readme_text[start + heading.len()..end]
    }

    pub fn subsection(&self, section_heading: &str, subsection_heading: &str) -> &str {
        let section = self.section(section_heading);
        let start = section.find(subsection_heading).unwrap_or_else(|| {
            panic!(
                "README section `{section_heading}` should contain subsection `{subsection_heading}`"
            );
        });
        let after_heading = &section[start..];
        let end = after_heading
            .find("\n### ")
            .map(|index| start + index)
            .unwrap_or(section.len());
        &section[start..end]
    }

    pub fn word_count(&self) -> usize {
        self.readme_text.split_whitespace().count()
    }

    pub fn materialize_operator_workspace(&self, root_dir: &std::path::Path) {
        self.workspace.materialize_operator_workspace(root_dir);
    }

    pub fn operator_file(&self, relative_path: &str) -> &str {
        self.workspace.operator_file(relative_path)
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}
