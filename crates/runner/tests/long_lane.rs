use std::{
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .canonicalize()
        .expect("fixtures dir should resolve")
}

fn unique_image_tag() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();

    format!("cockroach-migrate-runner-test-{timestamp}")
}

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_builds_and_runs_the_single_binary_runner_image() {
    let image_tag = unique_image_tag();
    let repo_root = repo_root();
    let repo_root_text = repo_root
        .to_str()
        .expect("repo root should be valid utf-8")
        .to_owned();
    let fixtures_dir = fixtures_dir();
    let fixture_mount = format!(
        "{}:/config:ro",
        fixtures_dir
            .to_str()
            .expect("fixtures dir should be valid utf-8")
    );

    let build_output = Command::new("docker")
        .args(["build", "-t", &image_tag, &repo_root_text])
        .output()
        .expect("docker build should start");

    assert!(
        build_output.status.success(),
        "docker build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let inspect_output = Command::new("docker")
        .args([
            "image",
            "inspect",
            &image_tag,
            "--format",
            "{{json .Config.Entrypoint}}",
        ])
        .output()
        .expect("docker image inspect should start");

    assert!(
        inspect_output.status.success(),
        "docker image inspect failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&inspect_output.stdout),
        String::from_utf8_lossy(&inspect_output.stderr)
    );

    assert_eq!(
        String::from_utf8(inspect_output.stdout)
            .expect("docker inspect output should be valid utf-8")
            .trim(),
        "[\"/usr/local/bin/runner\"]"
    );

    let validate_output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "-v",
            &fixture_mount,
            &image_tag,
            "validate-config",
            "--config",
            "/config/container-runner-config.yml",
        ])
        .output()
        .expect("docker validate-config should start");

    assert!(
        validate_output.status.success(),
        "docker validate-config failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&validate_output.stdout),
        String::from_utf8_lossy(&validate_output.stderr)
    );

    let validate_stdout =
        String::from_utf8(validate_output.stdout).expect("docker validate output should be utf-8");
    assert!(validate_stdout.contains("config=/config/container-runner-config.yml"));
    assert!(validate_stdout.contains("tls=/config/certs/server.crt+/config/certs/server.key"));

    let run_output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "-v",
            &fixture_mount,
            &image_tag,
            "run",
            "--config",
            "/config/container-runner-config.yml",
        ])
        .output()
        .expect("docker run should start");

    assert!(
        run_output.status.success(),
        "docker run failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );

    let run_stdout =
        String::from_utf8(run_output.stdout).expect("docker run output should be valid utf-8");
    assert!(run_stdout.contains("runner ready:"));
    assert!(run_stdout.contains("config=/config/container-runner-config.yml"));
    assert!(run_stdout.contains("webhook=0.0.0.0:8443"));

    let remove_output = Command::new("docker")
        .args(["image", "rm", "-f", &image_tag])
        .output()
        .expect("docker image removal should start");

    assert!(
        remove_output.status.success(),
        "docker image cleanup failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&remove_output.stdout),
        String::from_utf8_lossy(&remove_output.stderr)
    );
}
