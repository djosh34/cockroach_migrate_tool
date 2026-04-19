use std::ffi::OsString;

pub struct RunnerDockerContract;

pub struct RunnerRuntimeLaunch<'a> {
    pub image_tag: &'a str,
    pub container_name: &'a str,
    pub network_name: &'a str,
    pub auto_remove: bool,
    pub host_bind_ip: Option<&'a str>,
    pub host_port: u16,
    pub mounts: &'a [&'a str],
    pub extra_docker_args: &'a [&'a str],
    pub config_path: &'a str,
}

#[allow(dead_code)]
impl RunnerDockerContract {
    pub fn direct_runner_entrypoint_json() -> &'static str {
        "[\"/usr/local/bin/runner\"]"
    }

    pub fn documented_subcommands() -> &'static [&'static str] {
        &[
            "validate-config",
            "compare-schema",
            "render-postgres-setup",
            "render-helper-plan",
            "run",
        ]
    }

    pub fn assert_readme_documents_direct_build_and_run(docker_quick_start: &str) {
        assert!(
            docker_quick_start.contains("docker build -t cockroach-migrate-runner ."),
            "README Docker quick start must build the runner image directly from the repository root",
        );
        assert!(
            docker_quick_start.contains(
                "cockroach-migrate-runner \\\n  validate-config --config /config/runner.yml"
            ),
            "README Docker quick start must run `validate-config` directly through the runner image entrypoint",
        );
        assert!(
            docker_quick_start
                .contains("cockroach-migrate-runner \\\n  run --config /config/runner.yml"),
            "README Docker quick start must run `run --config` directly through the runner image entrypoint",
        );
    }

    pub fn assert_readme_has_no_wrapper_handoff(docker_quick_start: &str) {
        assert!(
            docker_quick_start.contains("There is no wrapper shell script in the user path."),
            "README Docker quick start must explicitly state that wrapper shell scripts are not part of the public container path",
        );
        for forbidden_marker in ["bash ", ".sh", "/bin/sh", "/bin/bash"] {
            assert!(
                !docker_quick_start.contains(forbidden_marker),
                "README Docker quick start must not hand the operator path off to `{forbidden_marker}`",
            );
        }
    }

    pub fn assert_cli_help_covers_documented_subcommands(help_output: &str) {
        for subcommand in Self::documented_subcommands() {
            assert!(
                help_output.contains(subcommand),
                "runner --help must include the Docker-documented subcommand `{subcommand}`",
            );
        }
    }

    pub fn assert_image_entrypoint_is_direct_runner(image_entrypoint_json: &str) {
        assert_eq!(
            image_entrypoint_json.trim(),
            Self::direct_runner_entrypoint_json(),
            "runner image must start the binary directly instead of handing off through a shell entrypoint",
        );
    }

    pub fn docker_build_image_args(image_tag: &str) -> Vec<OsString> {
        vec!["build".into(), "-t".into(), image_tag.into()]
    }

    pub fn docker_validate_config_args(
        image_tag: &str,
        config_mount: &str,
        config_path: &str,
        network_name: Option<&str>,
    ) -> Vec<OsString> {
        let mut args = vec!["run".into(), "--rm".into()];
        if let Some(network_name) = network_name {
            args.extend(["--network".into(), network_name.into()]);
        }
        args.extend([
            "-v".into(),
            config_mount.into(),
            image_tag.into(),
            "validate-config".into(),
            "--config".into(),
            config_path.into(),
        ]);
        args
    }

    pub fn docker_run_runtime_args(launch: RunnerRuntimeLaunch<'_>) -> Vec<OsString> {
        let mut args = vec![
            "run".into(),
            "-d".into(),
            "--name".into(),
            launch.container_name.into(),
            "--network".into(),
            launch.network_name.into(),
            "-p".into(),
            match launch.host_bind_ip {
                Some(host_bind_ip) => format!("{host_bind_ip}:{}:8443", launch.host_port),
                None => format!("{}:8443", launch.host_port),
            }
            .into(),
        ];
        if launch.auto_remove {
            args.insert(2, "--rm".into());
        }
        for extra_arg in launch.extra_docker_args {
            args.push((*extra_arg).into());
        }
        for mount in launch.mounts {
            args.push("-v".into());
            args.push((*mount).into());
        }
        args.extend([
            launch.image_tag.into(),
            "run".into(),
            "--config".into(),
            launch.config_path.into(),
        ]);
        args
    }
}
