use std::{fs, path::PathBuf};

pub struct TlsReferenceSurface {
    doc_text: String,
    readme_text: String,
}

impl TlsReferenceSurface {
    pub fn load() -> Self {
        let doc_path = repo_root().join("docs/tls-configuration.md");
        let readme_path = repo_root().join("README.md");

        let doc_text = fs::read_to_string(&doc_path).unwrap_or_else(|error| {
            panic!(
                "TLS reference doc should be readable at `{}`: {error}",
                doc_path.display(),
            )
        });
        let readme_text = fs::read_to_string(&readme_path).unwrap_or_else(|error| {
            panic!(
                "README should be readable at `{}`: {error}",
                readme_path.display(),
            )
        });

        Self {
            doc_text,
            readme_text,
        }
    }

    pub fn doc(&self) -> &str {
        &self.doc_text
    }

    pub fn readme(&self) -> &str {
        &self.readme_text
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}
