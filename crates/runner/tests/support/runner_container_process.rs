use std::{path::Path, sync::OnceLock};

use super::{
    container_running, docker_inspect_format, docker_logs, investigation_server_cert_path,
    run_command_capture,
    runner_docker_contract::{RunnerDockerContract, RunnerRuntimeLaunch},
};
use crate::nix_image_artifact_harness_support::NixImageArtifact;

pub(crate) struct RunnerContainerProcess {
    image_tag: String,
    container_name: String,
}

impl RunnerContainerProcess {
    pub(crate) fn start(network_name: &str, host_port: u16, config_path: &Path) -> Self {
        let image_tag = shared_runner_image_tag().to_owned();
        let container_name = format!("cockroach-migrate-runner-{}", super::unique_suffix());
        let cert_dir = investigation_server_cert_path()
            .parent()
            .expect("server cert should have a parent directory")
            .to_path_buf();
        let config_mount = format!("{}:/work/runner.yml:ro", config_path.display());
        let cert_mount = format!("{}:{}:ro", cert_dir.display(), cert_dir.display());
        run_command_capture(
            std::process::Command::new("docker").args(
                RunnerDockerContract::docker_run_runtime_args(RunnerRuntimeLaunch {
                    image_tag: &image_tag,
                    container_name: &container_name,
                    network_name,
                    auto_remove: false,
                    host_bind_ip: None,
                    host_port,
                    mounts: &[&config_mount, &cert_mount],
                    extra_docker_args: &["--add-host", "host.docker.internal:host-gateway"],
                    config_path: "/work/runner.yml",
                }),
            ),
            "docker run runner container",
        );

        Self {
            image_tag,
            container_name,
        }
    }

    pub(crate) fn assert_alive(&self) {
        if !container_running(&self.container_name) {
            panic!("runner container exited early\n{}", self.logs());
        }
    }

    pub(crate) fn kill(&self) {
        run_command_capture(
            std::process::Command::new("docker").args(["rm", "-f", &self.container_name]),
            "docker rm runner container",
        );
    }

    pub(crate) fn logs(&self) -> String {
        docker_logs(&self.container_name)
    }

    pub(crate) fn image_entrypoint_json(&self) -> String {
        run_command_capture(
            std::process::Command::new("docker").args(
                RunnerDockerContract::docker_inspect_image_entrypoint_args(&self.image_tag),
            ),
            "docker image inspect",
        )
        .trim()
        .to_owned()
    }

    pub(crate) fn container_ip(&self) -> String {
        docker_inspect_format(
            &self.container_name,
            "{{range.NetworkSettings.Networks}}{{.IPAddress}}{{end}}",
        )
    }
}

impl Drop for RunnerContainerProcess {
    fn drop(&mut self) {
        cleanup_if_present(
            std::process::Command::new("docker").args([
                "container",
                "inspect",
                &self.container_name,
            ]),
            std::process::Command::new("docker").args(["rm", "-f", &self.container_name]),
            "docker rm runner container",
        );
    }
}

fn shared_runner_image_tag() -> &'static str {
    static RUNNER_IMAGE_TAG: OnceLock<String> = OnceLock::new();

    RUNNER_IMAGE_TAG.get_or_init(|| {
        let image_tag = "cockroach-migrate-runner-e2e-local".to_owned();
        NixImageArtifact::runner().provision_image_tag(&image_tag, "runner e2e image");
        image_tag
    })
}

fn cleanup_if_present(
    probe: &mut std::process::Command,
    cleanup: &mut std::process::Command,
    context: &str,
) {
    let output = probe
        .output()
        .unwrap_or_else(|error| panic!("{context} probe should start: {error}"));
    if output.status.success() {
        run_command_capture(cleanup, context);
    }
}
