use std::{collections::BTreeSet, fs, path::PathBuf};

pub struct VerifyDockerContract {
    dockerfile_path: PathBuf,
    dockerfile_text: String,
}

impl VerifyDockerContract {
    pub fn load() -> Self {
        let dockerfile_path = verify_slice_root().join("Dockerfile");
        let dockerfile_text = fs::read_to_string(&dockerfile_path).unwrap_or_else(|error| {
            panic!(
                "verify image Dockerfile `{}` should be readable: {error}",
                dockerfile_path.display()
            )
        });

        Self {
            dockerfile_path,
            dockerfile_text,
        }
    }

    pub fn assert_verify_slice_owns_the_dockerfile(&self) {
        assert_eq!(
            self.dockerfile_path,
            verify_slice_root().join("Dockerfile"),
            "verify image Dockerfile should live directly under the verify-only source slice",
        );
    }

    pub fn assert_runtime_is_scratch_with_a_direct_binary_entrypoint(&self) {
        let runtime_stage = self
            .dockerfile_text
            .split("FROM ")
            .find(|stage| stage.starts_with("scratch"))
            .unwrap_or_else(|| {
                panic!("verify image Dockerfile must define a `FROM scratch` runtime stage")
            });
        let runtime_commands = runtime_stage
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        let copy_commands = runtime_commands
            .iter()
            .filter(|line| line.starts_with("COPY "))
            .copied()
            .collect::<Vec<_>>();

        assert!(
            runtime_commands
                .first()
                .is_some_and(|line| *line == "scratch AS runtime"),
            "verify image runtime stage must start from `scratch`",
        );
        assert!(
            runtime_commands
                .iter()
                .all(|line| !line.starts_with("ENTRYPOINT [\"/bin/sh\"")),
            "verify image runtime must not hand off through `/bin/sh`",
        );
        assert!(
            runtime_commands.iter().all(|line| !line.contains(".sh")),
            "verify image runtime must not rely on wrapper shell scripts",
        );
        assert!(
            runtime_commands
                .iter()
                .all(|line| !line.starts_with("RUN ")),
            "verify image runtime must not install extra runtime payload",
        );
        assert_eq!(
            copy_commands,
            vec!["COPY --from=builder /out/molt /usr/local/bin/molt"],
            "verify image runtime stage must copy only the compiled verify binary payload",
        );
        assert!(
            runtime_commands
                .iter()
                .any(|line| { line.starts_with("ENTRYPOINT [\"") && !line.contains("/bin/sh") }),
            "verify image runtime must start a binary directly with JSON entrypoint syntax",
        );
    }

    pub fn assert_image_entrypoint_is_direct_verify_surface(&self, image_entrypoint_json: &str) {
        assert_eq!(
            image_entrypoint_json.trim(),
            "[\"/usr/local/bin/molt\",\"verify-service\"]",
            "verify image must expose the verify-service command root directly from the entrypoint",
        );
    }

    pub fn assert_runtime_filesystem_is_minimal(&self, exported_paths: &[String]) {
        let actual_paths = exported_paths.iter().cloned().collect::<BTreeSet<_>>();
        let expected_paths = BTreeSet::from([
            String::from(".dockerenv"),
            String::from("dev/"),
            String::from("dev/console"),
            String::from("dev/pts/"),
            String::from("dev/shm/"),
            String::from("etc/"),
            String::from("etc/hostname"),
            String::from("etc/hosts"),
            String::from("etc/mtab"),
            String::from("etc/resolv.conf"),
            String::from("proc/"),
            String::from("sys/"),
            String::from("usr/"),
            String::from("usr/local/"),
            String::from("usr/local/bin/"),
            String::from("usr/local/bin/molt"),
        ]);

        assert_eq!(
            actual_paths, expected_paths,
            "verify image runtime filesystem must stay minimal and carry only the verify binary payload",
        );
    }

    pub fn assert_build_context_stays_within_the_verify_slice(&self) {
        for forbidden_marker in [
            "../",
            "Cargo.toml",
            "cargo build",
            "crates/",
            "/workspace/crates",
            "/home/",
        ] {
            assert!(
                !self.dockerfile_text.contains(forbidden_marker),
                "verify image Dockerfile must not couple back to repo-root or Rust workspace marker `{forbidden_marker}`",
            );
        }
        assert!(
            self.dockerfile_text.contains("COPY . ."),
            "verify image Dockerfile must copy only the verify-slice build context",
        );
        assert!(
            self.dockerfile_text.contains("go build"),
            "verify image Dockerfile must build from the vendored verify-only Go slice",
        );
    }

    pub fn assert_dockerfile_separates_go_module_and_source_cache_layers(&self) {
        for required_marker in [
            "# syntax=docker/dockerfile:1.7",
            "COPY go.mod go.sum ./",
            "go mod download",
            "--mount=type=cache,target=/go/pkg/mod",
            "--mount=type=cache,target=/root/.cache/go-build",
            "COPY . .",
            "CGO_ENABLED=0 GOOS=linux go build",
        ] {
            assert!(
                self.dockerfile_text.contains(required_marker),
                "verify image Dockerfile must contain `{required_marker}` to preserve Go dependency and build caches",
            );
        }

        assert_strict_order(
            &self.dockerfile_text,
            &[
                "COPY go.mod go.sum ./",
                "go mod download",
                "COPY . .",
                "CGO_ENABLED=0 GOOS=linux go build",
            ],
            "verify image Dockerfile",
        );
    }

    pub fn docker_build_image_args(image_tag: &str) -> Vec<String> {
        vec![
            String::from("build"),
            String::from("-t"),
            image_tag.to_owned(),
            String::from("-f"),
            verify_slice_root().join("Dockerfile").display().to_string(),
            verify_slice_root().display().to_string(),
        ]
    }
}

fn verify_slice_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("cockroachdb_molt/molt")
}

fn assert_strict_order(text: &str, ordered_markers: &[&str], dockerfile_label: &str) {
    let mut previous_position = None;
    for marker in ordered_markers {
        let position = text
            .find(marker)
            .unwrap_or_else(|| panic!("{dockerfile_label} must contain `{marker}`"));
        if let Some(previous_position) = previous_position {
            assert!(
                previous_position < position,
                "{dockerfile_label} must keep `{marker}` after the previous cache boundary marker",
            );
        }
        previous_position = Some(position);
    }
}
