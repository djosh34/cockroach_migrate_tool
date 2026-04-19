use std::{fs, path::PathBuf};

pub struct RepositoryReadme {
    text: String,
}

impl RepositoryReadme {
    pub fn load() -> Self {
        Self {
            text: fs::read_to_string(repository_readme_path())
                .expect("repository README should be readable"),
        }
    }

    pub fn setup_sql_cockroach_yaml_block(&self) -> String {
        let section_start = self
            .text
            .find("## Setup SQL Quick Start")
            .expect("README should include the setup-sql quick start");
        let section = &self.text[section_start..];
        let yaml_start = section
            .find("```yaml")
            .expect("setup-sql quick start should include a YAML example");
        let after_fence = &section[yaml_start + "```yaml".len()..];
        let yaml_end = after_fence
            .find("\n```")
            .expect("setup-sql Cockroach YAML example should close its code fence");

        after_fence[..yaml_end].trim().to_owned()
    }
}

fn repository_readme_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("README.md")
}
