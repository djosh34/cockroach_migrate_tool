use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::source_bootstrap_image_contract_support::SourceBootstrapImageContract;

pub struct SourceBootstrapImageHarness {
    image_tag: String,
}

impl SourceBootstrapImageHarness {
    pub fn start() -> Self {
        let harness = Self {
            image_tag: format!("cockroach-migrate-setup-sql-test-{}", unique_suffix()),
        };
        harness.build_image();
        harness
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
            "docker image inspect setup-sql entrypoint",
        )
    }

    pub fn assert_emit_cockroach_sql_output(&self) {
        let temp_dir = fresh_temp_dir();
        let config_path = temp_dir.join("cockroach-setup.yml");
        let ca_cert_path = temp_dir.join("ca.crt");
        fs::write(
            &config_path,
            fs::read_to_string(fixture_path("readme-cockroach-setup-config.yml"))
                .expect("README Cockroach setup config fixture should be readable"),
        )
        .expect("temp Cockroach setup config should be writable");
        fs::write(&ca_cert_path, b"dummy-ca\n").expect("temp CA cert fixture should be writable");

        let output = self.emit_cockroach_sql(&temp_dir, "/work/cockroach-setup.yml");

        assert!(
            output.starts_with("-- Source bootstrap SQL\n"),
            "setup-sql image must emit the rendered SQL header",
        );
        assert!(
            output.contains(
                "CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders"
            ),
            "setup-sql image must render the README mapping changefeed through the container entrypoint",
        );
        assert!(
            output.contains("INTO 'webhook-https://runner.example.internal:8443/ingest/app-a?ca_cert=ZHVtbXktY2EK'"),
            "setup-sql image must resolve the mounted CA cert from the config directory",
        );
    }

    pub fn emit_cockroach_sql(&self, mounted_dir: &Path, config_path: &str) -> String {
        let work_mount = format!("{}:/work:ro", mounted_dir.display());
        run_command_capture(
            Command::new("docker").args([
                "run",
                "--rm",
                "-v",
                &work_mount,
                &self.image_tag,
                "emit-cockroach-sql",
                "--config",
                config_path,
            ]),
            "docker run setup-sql emit-cockroach-sql",
        )
    }

    fn build_image(&self) {
        run_command_capture(
            Command::new("docker").args(SourceBootstrapImageContract::docker_build_image_args(
                &self.image_tag,
            )),
            "docker build setup-sql image",
        );
    }
}

impl Drop for SourceBootstrapImageHarness {
    fn drop(&mut self) {
        let output = Command::new("docker")
            .args(["image", "inspect", &self.image_tag])
            .output()
            .unwrap_or_else(|error| {
                panic!("docker image rm setup-sql image probe should start: {error}")
            });
        if output.status.success() {
            run_command_capture(
                Command::new("docker").args(["image", "rm", "-f", &self.image_tag]),
                "docker image rm setup-sql image",
            );
        }
    }
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn fresh_temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "setup-sql-image-contract-{}",
        unique_suffix()
    ));
    fs::create_dir_all(&dir).expect("setup-sql image temp dir should be created");
    dir
}

fn unique_suffix() -> String {
    static UNIQUE_SUFFIX_COUNTER: AtomicU64 = AtomicU64::new(0);

    format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos(),
        UNIQUE_SUFFIX_COUNTER.fetch_add(1, Ordering::Relaxed),
    )
}

fn run_command_capture(command: &mut Command, context: &str) -> String {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{context} should start: {error}"));
    assert!(
        output.status.success(),
        "{context} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("command stdout should be utf-8")
}
