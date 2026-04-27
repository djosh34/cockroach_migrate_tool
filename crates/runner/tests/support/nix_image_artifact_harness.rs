use std::{path::PathBuf, process::Command};

pub(crate) struct NixImageArtifact<'a> {
    package_attr: &'a str,
    loaded_image_ref: &'a str,
}

impl<'a> NixImageArtifact<'a> {
    pub(crate) const fn new(package_attr: &'a str, loaded_image_ref: &'a str) -> Self {
        Self {
            package_attr,
            loaded_image_ref,
        }
    }

    pub(crate) fn provision_image_tag(&self, image_tag: &str, context: &str) {
        let package_selector = format!(".#{}", self.package_attr);
        let build_output = run_command_capture(
            Command::new("nix")
                .current_dir(repo_root())
                .args(["build", "--no-link", "--print-out-paths", &package_selector]),
            &format!("nix build {package_selector} for {context}"),
        );
        let image_archive_path = build_output.trim();

        run_command_capture(
            Command::new("docker").args(["load", "-i", image_archive_path]),
            &format!("docker load {context}"),
        );
        run_command_capture(
            Command::new("docker").args(["tag", self.loaded_image_ref, image_tag]),
            &format!("docker tag {context}"),
        );
    }
}

pub(crate) fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}

pub(crate) fn run_command_capture(command: &mut Command, context: &str) -> String {
    let (stdout, _) = run_command_output(command, context);
    stdout
}

pub(crate) fn run_command_output(command: &mut Command, context: &str) -> (String, String) {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{context} should start: {error}"));
    assert!(
        output.status.success(),
        "{context} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (
        String::from_utf8(output.stdout).expect("command stdout should be utf-8"),
        String::from_utf8(output.stderr).expect("command stderr should be utf-8"),
    )
}
