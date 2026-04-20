use std::{
    path::PathBuf,
    process::Command,
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) fn setup_sql_image_ref() -> &'static str {
    static SETUP_SQL_IMAGE_REF: OnceLock<String> = OnceLock::new();

    SETUP_SQL_IMAGE_REF.get_or_init(|| {
        let image_ref = format!("cockroach-migrate-setup-sql-novice-{}", unique_suffix());
        let output = Command::new("docker")
            .args([
                "build",
                "-t",
                &image_ref,
                "-f",
                &repo_root()
                    .join("crates/setup-sql/Dockerfile")
                    .display()
                    .to_string(),
                &repo_root().display().to_string(),
            ])
            .output()
            .unwrap_or_else(|error| {
                panic!("docker build setup-sql novice image should start: {error}")
            });
        assert!(
            output.status.success(),
            "docker build setup-sql novice image failed with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        image_ref
    })
}

pub(crate) fn runner_image_ref() -> &'static str {
    static RUNNER_IMAGE_REF: OnceLock<String> = OnceLock::new();

    RUNNER_IMAGE_REF.get_or_init(|| {
        let image_ref = format!("cockroach-migrate-runner-novice-{}", unique_suffix());
        let output = Command::new("docker")
            .args([
                "build",
                "-t",
                &image_ref,
                &repo_root().display().to_string(),
            ])
            .output()
            .unwrap_or_else(|error| {
                panic!("docker build runner novice image should start: {error}")
            });
        assert!(
            output.status.success(),
            "docker build runner novice image failed with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        image_ref
    })
}

pub(crate) fn verify_image_ref() -> &'static str {
    static VERIFY_IMAGE_REF: OnceLock<String> = OnceLock::new();

    VERIFY_IMAGE_REF.get_or_init(|| {
        let image_ref = format!("cockroach-migrate-verify-novice-{}", unique_suffix());
        let verify_root = repo_root().join("cockroachdb_molt/molt");
        let output = Command::new("docker")
            .args([
                "build",
                "-t",
                &image_ref,
                "-f",
                &verify_root.join("Dockerfile").display().to_string(),
                &verify_root.display().to_string(),
            ])
            .output()
            .unwrap_or_else(|error| {
                panic!("docker build verify novice image should start: {error}")
            });
        assert!(
            output.status.success(),
            "docker build verify novice image failed with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        image_ref
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}

fn unique_suffix() -> String {
    static UNIQUE_SUFFIX_COUNTER: AtomicU64 = AtomicU64::new(0);

    format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos(),
        UNIQUE_SUFFIX_COUNTER.fetch_add(1, Ordering::Relaxed),
    )
}
