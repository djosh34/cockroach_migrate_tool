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
