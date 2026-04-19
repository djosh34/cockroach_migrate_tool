use std::{ffi::OsString, fs, path::PathBuf};

#[path = "../../../runner/tests/support/rust_workspace_image_cache_contract.rs"]
mod rust_workspace_image_cache_contract_support;

use rust_workspace_image_cache_contract_support::{
    RustWorkspaceImageCacheContract, RustWorkspaceImageCacheExpectation,
};

pub struct SourceBootstrapImageContract {
    dockerfile_path: PathBuf,
    dockerfile_text: String,
}

impl SourceBootstrapImageContract {
    pub fn load() -> Self {
        let dockerfile_path = source_bootstrap_slice_root().join("Dockerfile");
        let dockerfile_text = fs::read_to_string(&dockerfile_path).unwrap_or_else(|error| {
            panic!(
                "setup-sql image Dockerfile `{}` should be readable: {error}",
                dockerfile_path.display()
            )
        });

        Self {
            dockerfile_path,
            dockerfile_text,
        }
    }

    pub fn assert_setup_slice_owns_the_dockerfile(&self) {
        assert_eq!(
            self.dockerfile_path,
            source_bootstrap_slice_root().join("Dockerfile"),
            "setup-sql image Dockerfile should live directly under the setup slice",
        );
    }

    pub fn assert_runtime_is_scratch_with_a_direct_binary_entrypoint(&self) {
        let runtime_stage = self
            .dockerfile_text
            .split("FROM ")
            .find(|stage| stage.starts_with("scratch"))
            .unwrap_or_else(|| {
                panic!(
                    "setup-sql image Dockerfile must define a `FROM scratch` runtime stage"
                )
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
            "setup-sql image runtime stage must start from `scratch`",
        );
        assert!(
            self.dockerfile_text.contains("ARG TARGETARCH"),
            "setup-sql builder stage must derive its musl target from TARGETARCH",
        );
        for required_target in [
            "x86_64-unknown-linux-musl",
            "aarch64-unknown-linux-musl",
            "unsupported TARGETARCH",
        ] {
            assert!(
                self.dockerfile_text.contains(required_target),
                "setup-sql builder stage must handle `{required_target}` explicitly",
            );
        }
        assert!(
            self.dockerfile_text
                .contains("-p setup-sql --bin setup-sql"),
            "setup-sql image must build the dedicated setup-sql binary",
        );
        assert_eq!(
            copy_commands,
            vec![
                "COPY --from=builder /setup-sql/setup-sql /usr/local/bin/setup-sql",
            ],
            "setup-sql image runtime stage must copy only the compiled setup-sql binary",
        );
        assert!(
            runtime_commands
                .iter()
                .all(|line| !line.starts_with("RUN ")),
            "setup-sql image runtime stage must not install extra runtime payload",
        );
        assert!(
            runtime_commands.contains(&"ENTRYPOINT [\"/usr/local/bin/setup-sql\"]"),
            "setup-sql image runtime stage must start the binary directly",
        );
    }

    pub fn assert_image_entrypoint_is_direct_setup_sql(&self, image_entrypoint_json: &str) {
        assert_eq!(
            image_entrypoint_json.trim(),
            "[\"/usr/local/bin/setup-sql\"]",
            "setup-sql image must invoke the binary directly instead of using a shell wrapper",
        );
    }

    pub fn assert_dockerfile_uses_dependency_first_rust_cache_layers(&self) {
        RustWorkspaceImageCacheContract::assert_dependency_first_layers(
            &self.dockerfile_text,
            RustWorkspaceImageCacheExpectation {
                dockerfile_label: "setup-sql image Dockerfile",
                build_command:
                    "cargo build --locked --release --target \"${RUST_TARGET}\" -p setup-sql --bin setup-sql",
            },
        );
    }

    pub fn docker_build_image_args(image_tag: &str) -> Vec<OsString> {
        vec![
            OsString::from("build"),
            OsString::from("-t"),
            OsString::from(image_tag),
            OsString::from("-f"),
            source_bootstrap_slice_root()
                .join("Dockerfile")
                .into_os_string(),
            repo_root().into_os_string(),
        ]
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn source_bootstrap_slice_root() -> PathBuf {
    repo_root().join("crates/setup-sql")
}
