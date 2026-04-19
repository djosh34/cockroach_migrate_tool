use std::{
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::verify_docker_contract_support::VerifyDockerContract;

pub struct VerifyImageArtifactHarness {
    image_tag: String,
}

impl VerifyImageArtifactHarness {
    pub fn start() -> Self {
        let harness = Self {
            image_tag: format!("cockroach-migrate-verify-test-{}", unique_suffix()),
        };
        harness.build_verify_image();
        harness
    }

    pub fn assert_image_exists(&self) {
        run_command_capture(
            Command::new("docker").args([
                "image",
                "inspect",
                &self.image_tag,
                "--format",
                "{{.Id}}",
            ]),
            "docker image inspect verify image",
        );
    }

    pub fn image_entrypoint_json(&self) -> String {
        run_command_capture(
            Command::new("docker").args([
                "image",
                "inspect",
                &self.image_tag,
                "--format",
                "{{json .Config.Entrypoint}}",
            ]),
            "docker image inspect verify image entrypoint",
        )
    }

    pub fn help_output(&self) -> String {
        run_command_capture(
            Command::new("docker").args(["run", "--rm", &self.image_tag, "--help"]),
            "docker run verify image --help",
        )
    }

    pub fn exported_runtime_paths(&self) -> Vec<String> {
        let container_id = run_command_capture(
            Command::new("docker").args(["create", &self.image_tag]),
            "docker create verify image",
        );
        let container_id = container_id.trim().to_owned();

        let output = Command::new("bash")
            .args([
                "-lc",
                &format!(
                    "docker export {container_id} | tar -tf -",
                    container_id = shell_escape(&container_id)
                ),
            ])
            .output()
            .unwrap_or_else(|error| panic!("docker export verify image should start: {error}"));

        let cleanup_output = Command::new("docker")
            .args(["rm", "-f", &container_id])
            .output()
            .unwrap_or_else(|error| {
                panic!("docker rm verify image export container should start: {error}")
            });
        assert!(
            cleanup_output.status.success(),
            "docker rm verify image export container failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&cleanup_output.stdout),
            String::from_utf8_lossy(&cleanup_output.stderr)
        );

        assert!(
            output.status.success(),
            "docker export verify image failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        String::from_utf8(output.stdout)
            .expect("docker export verify image output should be utf-8")
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect()
    }

    fn build_verify_image(&self) {
        run_command_capture(
            Command::new("docker").args(VerifyDockerContract::docker_build_image_args(
                &self.image_tag,
            )),
            "docker build verify image",
        );
    }
}

impl Drop for VerifyImageArtifactHarness {
    fn drop(&mut self) {
        let output = Command::new("docker")
            .args(["image", "inspect", &self.image_tag])
            .output()
            .unwrap_or_else(|error| {
                panic!("docker image inspect verify image should start: {error}")
            });
        if output.status.success() {
            run_command_capture(
                Command::new("docker").args(["rmi", "-f", &self.image_tag]),
                "docker rmi verify image",
            );
        }
    }
}

fn unique_suffix() -> String {
    static UNIQUE_SUFFIX_COUNTER: AtomicU64 = AtomicU64::new(0);

    format!(
        "{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos(),
        UNIQUE_SUFFIX_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn run_command_capture(command: &mut Command, context: &str) -> String {
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
    String::from_utf8(output.stdout).expect("command stdout should be utf-8")
}

fn shell_escape(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
