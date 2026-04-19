use std::{fs, path::PathBuf};

pub struct RepoLicenseContract {
    cargo_toml_text: String,
    license_text: String,
    notices_text: String,
    readme_text: String,
}

impl RepoLicenseContract {
    pub fn load() -> Self {
        let root = repo_root();
        let cargo_toml_text = read_required_file(root.join("Cargo.toml"));
        let license_text = read_required_file(root.join("LICENSE"));
        let notices_text = read_required_file(root.join("THIRD_PARTY_NOTICES"));
        let readme_text = read_required_file(root.join("README.md"));

        Self {
            cargo_toml_text,
            license_text,
            notices_text,
            readme_text,
        }
    }

    pub fn assert_root_declares_proprietary_rust_workspace_and_apache_vendored_component(&self) {
        for required_marker in [
            "All Rights Reserved - Joshua Azimullah",
            "Rust workspace",
            "cockroachdb_molt/molt",
            "Apache License, Version 2.0",
            "cockroachdb_molt/molt/LICENSE",
        ] {
            assert!(
                self.license_text.contains(required_marker),
                "root LICENSE must contain `{required_marker}`",
            );
        }

        for required_marker in [
            "LicenseRef-Proprietary",
            "license = \"LicenseRef-Proprietary\"",
        ] {
            assert!(
                self.cargo_toml_text.contains(required_marker),
                "Cargo.toml must mark the Rust workspace as proprietary with `{required_marker}`",
            );
        }

        for required_marker in [
            "cockroachdb_molt/molt",
            "Apache License, Version 2.0",
            "cockroachdb_molt/molt/LICENSE",
            "Rust workspace",
            "All Rights Reserved - Joshua Azimullah",
        ] {
            assert!(
                self.notices_text.contains(required_marker),
                "THIRD_PARTY_NOTICES must contain `{required_marker}`",
            );
            assert!(
                self.readme_text.contains(required_marker),
                "README.md must document `{required_marker}`",
            );
        }
    }

    pub fn assert_workspace_tls_crypto_backend_is_explicitly_ring_only(&self) {
        assert!(
            self.cargo_toml_text.contains(
                "rustls = { version = \"0.23.34\", default-features = false, features = [\"ring\"] }"
            ),
            "Cargo.toml must disable rustls default crypto backends and keep the workspace on the explicit `ring` provider only",
        );
        assert!(
            !self.cargo_toml_text.contains("rustls = { version = \"0.23.34\", features = [\"ring\"] }"),
            "Cargo.toml must not leave rustls default features enabled alongside the `ring` feature",
        );
        assert!(
            self.cargo_toml_text.contains(
                "tokio-rustls = { version = \"0.26.4\", default-features = false, features = [\"ring\", \"tls12\"] }"
            ),
            "Cargo.toml must disable tokio-rustls default crypto features and keep only the explicit ring/tls12 surface",
        );
        assert!(
            !self.cargo_toml_text.contains("tokio-rustls = \"0.26.4\""),
            "Cargo.toml must not leave tokio-rustls default features enabled",
        );
    }
}

fn read_required_file(path: PathBuf) -> String {
    fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "required repo file `{}` should be readable: {error}",
            path.display()
        )
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}
