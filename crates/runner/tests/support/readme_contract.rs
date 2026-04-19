use std::{fs, path::PathBuf};

pub struct RepositoryReadme {
    text: String,
}

pub struct ReadmeSection<'a> {
    text: &'a str,
}

impl RepositoryReadme {
    pub fn load() -> Self {
        Self {
            text: fs::read_to_string(repository_readme_path())
                .expect("repository README should be readable"),
        }
    }

    pub fn assert_contains(&self, needle: &str, message: &str) {
        assert!(self.text.contains(needle), "{message}");
    }

    pub fn assert_in_order(&self, phrases: &[&str], message: &str) {
        let mut offsets = phrases.iter().map(|phrase| self.offset_of(phrase));
        let Some(mut previous_offset) = offsets.next() else {
            panic!("phrase order assertions require at least one phrase");
        };

        for offset in offsets {
            assert!(previous_offset < offset, "{message}");
            previous_offset = offset;
        }
    }

    pub fn docker_quick_start(&self) -> ReadmeSection<'_> {
        ReadmeSection {
            text: self.section("## Docker Quick Start"),
        }
    }

    pub fn source_bootstrap_quick_start(&self) -> ReadmeSection<'_> {
        ReadmeSection {
            text: self.section("## Source Bootstrap Quick Start"),
        }
    }

    fn offset_of(&self, phrase: &str) -> usize {
        self.text
            .find(phrase)
            .unwrap_or_else(|| panic!("README must contain `{phrase}`"))
    }

    fn section(&self, heading: &str) -> &str {
        let start = self.offset_of(heading);
        let after_heading = &self.text[start + heading.len()..];
        let end = after_heading.find("\n## ").unwrap_or(after_heading.len());
        &after_heading[..end]
    }
}

impl ReadmeSection<'_> {
    pub fn assert_contains(&self, needle: &str, message: &str) {
        assert!(self.text.contains(needle), "{message}");
    }

    pub fn contains(&self, needle: &str) -> bool {
        self.text.contains(needle)
    }
}

fn repository_readme_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("README.md")
}
