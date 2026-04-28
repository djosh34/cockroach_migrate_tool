use std::{path::Path, process::Command};

use crate::nix_image_artifact_harness_support::run_command_capture;

pub(crate) struct DockerImageContainer {
    container_id: String,
    image_label: String,
}

impl DockerImageContainer {
    pub(crate) fn create(image_tag: &str, image_label: &str) -> Self {
        let container_id = run_command_capture(
            Command::new("docker").args(["create", image_tag]),
            &format!("docker create {image_label}"),
        );

        Self {
            container_id: container_id.trim().to_owned(),
            image_label: image_label.to_owned(),
        }
    }

    pub(crate) fn copy_file(&self, container_path: &str, host_path: &Path, context: &str) {
        let host_path = host_path
            .to_str()
            .expect("temporary binary path should be valid utf-8");
        let copy_result = Command::new("docker")
            .args([
                "cp",
                &format!("{}:{container_path}", self.container_id),
                host_path,
            ])
            .output()
            .unwrap_or_else(|error| panic!("{context} should start: {error}"));
        assert!(
            copy_result.status.success(),
            "{context} failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&copy_result.stdout),
            String::from_utf8_lossy(&copy_result.stderr)
        );
    }
}

impl Drop for DockerImageContainer {
    fn drop(&mut self) {
        run_command_capture(
            Command::new("docker").args(["rm", "-f", &self.container_id]),
            &format!("docker rm {} temporary container", self.image_label),
        );
    }
}
