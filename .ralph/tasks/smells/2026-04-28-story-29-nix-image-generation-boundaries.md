## Smell Set: 2026-04-28-story-29-nix-image-generation-boundaries <status>completed</status> <passes>true</passes>

<plan>.ralph/tasks/story-29-migrate-to-nix-crane/03-task-migrate-docker-image-generation-to-nix_plans/2026-04-28-nix-image-generation-boundaries-follow-up-plan.md</plan>

Please refer to skill 'improve-code-boundaries' to see what smells there are.

Inside dirs:
- `crates/runner/tests/support`
- `.github/workflows`
- `.`

Solve each smell:

---
- [x] Smell 3, Wrong Place-ism
Image assembly knowledge lives in the wrong modules. `flake.nix` owns binary builds, Dockerfiles own production image assembly, workflow metadata points at Dockerfile paths, and test harnesses know how to build those Dockerfiles directly. That forces the repo to keep two packaging truths alive at once.

code:
```nix
runner = craneLib.buildPackage {
  inherit cargoArtifacts;
  pname = runnerPname;
  version = runnerVersion;
  src = cargoSrc;
  strictDeps = true;
  cargoExtraArgs = "-p runner";
  doCheck = false;
};
```

```dockerfile
FROM scratch AS runtime

COPY --from=builder /runner/runner /usr/local/bin/runner

ENTRYPOINT ["/usr/local/bin/runner"]
```

```yaml
{
  "image_id": "runner",
  "quay_repository": "runner",
  "ghcr_repository": "cockroach-migrate-runner",
  "dockerfile": "./Dockerfile",
  "context": ".",
  "manifest_key": "runner_image_ref",
  "artifact_name": "published-image-runner",
  "cache_scope": "publish-image-runner",
  "build_kind": "rust-workspace-musl"
}
```

```rust
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
```

---
- [x] Smell 13, Multiple functions with large overlap
Runner and verify image harnesses repeat the same image-artifact lifecycle with only small differences in tag names and image-specific assertions. A third copy exists in `published_image_refs.rs`. The shared concern is "load a local test image artifact into Docker," but the code repeats build/create/export/remove plumbing in multiple files.

code:
```rust
fn build_runner_image(&self) {
    run_command_capture(
        Command::new("docker")
            .args(RunnerDockerContract::docker_build_image_args(
                &self.image_tag,
            ))
            .arg(repo_root()),
        "docker build runner image",
    );
}
```

```rust
fn build_verify_image(&self) {
    run_command_capture(
        Command::new("docker").args(
            crate::verify_docker_contract_support::docker_build_image_args(&self.image_tag),
        ),
        "docker build verify image",
    );
}
```

```rust
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
```

---
- [x] Smell 10, Remove The Damn Helpers
There is Dockerfile-shape contract support that no longer has an honest caller or boundary. It tests implementation text markers rather than runtime behavior, and it appears to be dead code. Keeping it around increases the chance that image migration preserves the Dockerfile as an implementation detail instead of removing it.

code:
```rust
pub struct RustWorkspaceImageCacheExpectation<'a> {
    pub dockerfile_label: &'a str,
    pub build_command: &'a str,
}

pub struct RustWorkspaceImageCacheContract;

impl RustWorkspaceImageCacheContract {
    pub fn assert_dependency_first_layers(
        dockerfile_text: &str,
        expectation: RustWorkspaceImageCacheExpectation<'_>,
    ) {
        for required_marker in [
            "# syntax=docker/dockerfile:1.7",
            "FROM rust:1.93-bookworm AS chef",
            "RUN cargo chef prepare --recipe-path recipe.json",
            "cargo chef cook --locked --release --target \"${RUST_TARGET}\" --recipe-path recipe.json",
        ] {
            assert!(
                dockerfile_text.contains(required_marker),
                "{} must contain `{required_marker}` to keep Rust dependency planning and build caching explicit",
                expectation.dockerfile_label,
            );
        }
    }
}
```
