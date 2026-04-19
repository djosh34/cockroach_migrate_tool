# Plan: Massively Improve Image Build Speed With Shared Cache Boundaries

## References

- Task:
  - `.ralph/tasks/story-21-github-workflows-image-publish/02-task-massively-improve-image-build-speed-with-docker-layer-and-build-cache-reuse.md`
- Workflow under change:
  - `.github/workflows/publish-images.yml`
- Dockerfiles under change:
  - `Dockerfile`
  - `crates/setup-sql/Dockerfile`
  - `cockroachdb_molt/molt/Dockerfile`
- Existing workflow contract boundary:
  - `crates/runner/tests/ci_contract.rs`
  - `crates/runner/tests/support/github_workflow_contract.rs`
  - `crates/runner/tests/support/published_image_contract.rs`
- Existing Docker/image contract boundaries:
  - `crates/runner/tests/image_contract.rs`
  - `crates/runner/tests/support/runner_docker_contract.rs`
  - `crates/setup-sql/tests/image_contract.rs`
  - `crates/setup-sql/tests/support/source_bootstrap_image_contract.rs`
  - `crates/runner/tests/verify_image_contract.rs`
  - `crates/runner/tests/support/verify_docker_contract.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is the approval source for the public workflow behavior and testing priorities in this turn.
- This turn is planning-only because the task file had no execution marker.
- Execution must stay in strict red/green order:
  - one failing public contract at a time
  - one minimal implementation slice at a time
  - refactor only after green
- `make test-long` is not a default task gate here.
  - Only run it during execution if the implementation changes the ultra-long lane itself or proves the long lane is part of this task boundary.
- If the first execution slices prove that the chosen cache topology cannot honestly express the required behavior without inventing fake contracts or fragile string snapshots, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.
- If execution discovers that a trustworthy native `arm64` runner path cannot be expressed from repo-owned configuration and a real trusted runner label, the task must explicitly reject that path instead of pretending it exists.

## Current State Summary

- `.github/workflows/publish-images.yml` has one `validate` job and one `publish` job.
- `validate` currently runs `make check` and `make test`, but it always cold-installs Rust/PostgreSQL and does not restore/save any dependency or build cache.
- `publish` currently builds and pushes the three images sequentially in one job.
  - No `cache-from`
  - No `cache-to`
  - No shared cache scope contract
  - No parallelism across the three independent image targets
- Both Rust Dockerfiles destroy cache reuse immediately:
  - `COPY . .` happens before dependency resolution and compilation
  - `rustup target add` and `cargo build` happen in the same late layer
  - the runner and setup-sql Dockerfiles duplicate the same cache-hostile structure
- The verify Go Dockerfile is also cache-hostile:
  - `COPY . .` happens before module download/build
  - there is no separated `go.mod`/`go.sum` dependency layer
  - there are no Go module/build cache mounts
- The repo already has strong public contract seams for this work.
  - workflow behavior is asserted through `GithubWorkflowContract`
  - image runtime behavior is asserted through per-image Docker contract helpers
  - what is missing is cache-boundary coverage, not an entirely new test framework

## Interface And Boundary Decisions

- Keep `crates/runner/tests/ci_contract.rs` as the thin public specification layer.
  - It should continue to say what the workflow guarantees, not parse YAML itself.
- Extend `GithubWorkflowContract` to own the workflow cache/reuse behavior.
  - It should assert the host-side validation cache boundary.
  - It should assert the BuildKit cache plumbing for image builds.
  - It should assert the job topology chosen to speed up image publication.
- Introduce one new support owner for build-target topology.
  - Preferred file: `crates/runner/tests/support/image_build_target_contract.rs`
  - It should own the canonical per-image build facts now duplicated across workflow assertions:
    - image id
    - dockerfile path
    - build context
    - repository env var
    - manifest output key
    - cache scope id
    - build kind (`rust-workspace-musl` or `verify-go`)
- Keep published-image registry coordinates in `PublishedImageContract`.
  - Do not overload it with cache-stage details.
  - Build topology and published repository naming are related but different boundaries.
- Extract the shared Rust Docker cache assertions into one reusable helper instead of duplicating them in two image-specific contract files.
  - Preferred file: `crates/runner/tests/support/rust_workspace_image_cache_contract.rs`
  - `RunnerDockerContract` and `SourceBootstrapImageContract` should delegate shared Rust cache-shape checks into it.
- Prefer one honest publish topology that improves wall-clock time beyond cache reuse.
  - Preferred direction: validate first, then parallelize the three independent image publishes through a matrix or three dedicated jobs.
  - The manifest publication step can remain downstream and assemble refs after the parallel image jobs finish.
- Keep all failure modes loud.
  - Missing cache artifacts
  - missing manifest artifacts
  - unsupported `TARGETARCH`
  - missing runner capabilities
  - unexpected workflow drift

## Public Contract To Establish

- One fast contract fails if the workflow does not restore/save host-side Rust dependency/build caches for the validation lane.
- One fast contract fails if the workflow does not define per-image BuildKit cache scopes through a shared build-target contract.
- One fast contract fails if each published image does not use both `--cache-from` and `--cache-to` through explicit `docker buildx build` flags.
- One fast contract fails if the publish topology remains one long sequential bottleneck when the three image targets are independent.
- One fast contract fails if the workflow loses the existing safety boundaries while being optimized:
  - trusted `push` to `main` only
  - validation before publish
  - explicit publish gating
  - least-privilege permissions
  - no hidden credential leakage
- One fast contract fails if the Rust Dockerfiles do not separate:
  - dependency planning input
  - dependency cooking/build cache layers
  - final source copy and binary build
- One fast contract fails if the Rust Dockerfiles do not explicitly evaluate `cargo-chef`.
  - Preferred plan: adopt `cargo-chef` unless the first real RED/GREEN slice proves it cannot honestly support this workspace/target shape.
- One fast contract fails if the Go verify Dockerfile does not separate module-resolution input from later source-copy/build input and use BuildKit cache mounts for module/build reuse.
- One fast contract fails if later edits can silently revert the cache topology by removing the expected dependency-only layers or cache flags.
- One fast contract fails if the task outcome does not explicitly record the native `arm64` evaluation decision:
  - adopted with a trusted native runner path
  - or rejected with a clear reason

## Improve-Code-Boundaries Focus

- Primary smell 1:
  - build-target knowledge lives in the wrong place and in too many places
  - dockerfile/context/repository/cache facts are partly embedded in YAML checks, partly in helper functions, and partly in ad hoc string literals
- Primary cleanup 1:
  - move build-target metadata into one support owner and consume it from the workflow contract
- Primary smell 2:
  - the two Rust image Docker contracts each own near-identical cache-topology knowledge
- Primary cleanup 2:
  - extract one shared Rust workspace cache contract helper and remove duplicate Rust Docker cache assertions from image-specific helpers
- Primary smell 3:
  - the workflow currently treats publish as one giant serial script even though the targets are separate products
- Primary cleanup 3:
  - flatten the publish topology into parallel target jobs backed by one shared contract instead of three hand-copied sequential build sections
- Smells to avoid during execution:
  - expanding `PublishedImageContract` into a mixed build-plus-registry god object
  - adding another ad hoc workflow parser helper beside `GithubWorkflowContract`
  - introducing cache behavior only in YAML with no public contract coverage
  - keeping `COPY . .` before dependency work and then claiming the build is cache-aware

## Files And Structure To Add Or Change

- [x] `.github/workflows/publish-images.yml`
  - add host-side validation cache restore/save
  - add per-image BuildKit cache import/export
  - replace the sequential publish bottleneck with a parallel publish topology if the first TDD slices support it
  - keep manifest publication and existing trust/permission guardrails honest
- [x] `Dockerfile`
  - restructure runner build for dependency-first caching
  - adopt or explicitly reject `cargo-chef`
  - keep the scratch runtime contract intact
- [x] `crates/setup-sql/Dockerfile`
  - mirror the Rust cache-friendly structure for setup-sql
  - keep the scratch runtime contract intact
- [x] `cockroachdb_molt/molt/Dockerfile`
  - split Go module dependency work from later source build work
  - add Go module/build cache mounts
  - keep the scratch runtime contract intact
- [x] `crates/runner/tests/ci_contract.rs`
  - add behavior-level workflow cache and topology assertions
- [x] `crates/runner/tests/support/github_workflow_contract.rs`
  - add helpers/assertions for:
    - host-side validation cache boundaries
    - shared image-build cache scopes
    - parallel publish topology
    - cache-backed publish commands
    - native `arm64` decision recording
- [x] `crates/runner/tests/support/image_build_target_contract.rs`
  - new canonical build-target metadata owner
- [x] `crates/runner/tests/support/rust_workspace_image_cache_contract.rs`
  - new shared Rust Docker cache contract owner
- [x] `crates/runner/tests/support/runner_docker_contract.rs`
  - delegate shared Rust cache-shape checks to the shared helper
- [x] `crates/setup-sql/tests/support/source_bootstrap_image_contract.rs`
  - delegate shared Rust cache-shape checks to the shared helper
- [x] `crates/runner/tests/support/verify_docker_contract.rs`
  - add explicit Go dependency/cache-layer assertions
- [x] Task markdown if needed during execution
  - record the explicit native `arm64` runner decision and the `cargo-chef` decision in the task outcome before completion

## TDD Execution Order

### Slice 1: Tracer Bullet For Shared Build-Target Cache Ownership

- [x] RED: add one failing workflow contract that requires a shared image-build target owner with explicit cache scopes for runner, setup-sql, and verify
- [x] GREEN: add `image_build_target_contract.rs` with the smallest truthful metadata surface and wire `GithubWorkflowContract` to use it
- [x] REFACTOR: remove duplicated dockerfile/context/output/cache-scope literals from workflow helper code

### Slice 2: Runner Rust Dockerfile Dependency-First Cache Shape

- [x] RED: add one failing runner Docker contract that requires:
  - dependency planning before full source copy
  - explicit dependency cook/build reuse stages
  - preservation of the scratch runtime boundary
- [x] GREEN: implement the minimal runner Dockerfile restructure to satisfy that contract
- [x] REFACTOR: extract shared Rust cache assertions once the shape is proven

### Slice 3: Setup-SQL Rust Dockerfile Uses The Same Shared Cache Contract

- [x] RED: add the next failing setup-sql Docker contract for the same dependency-first Rust cache shape
- [x] GREEN: implement the minimal setup-sql Dockerfile restructure to satisfy that contract
- [x] REFACTOR: move shared Rust cache-topology logic into `rust_workspace_image_cache_contract.rs` and delete duplicate assertions from the two image-specific helpers

### Slice 4: Verify Go Dockerfile Separates Module Cache From Source Cache

- [x] RED: add one failing verify Docker contract that requires:
  - `go.mod`/`go.sum`-first dependency resolution
  - module/build cache mounts
  - later source copy/build
  - preservation of the scratch runtime boundary
- [x] GREEN: implement the minimal Go Dockerfile restructure to satisfy that contract
- [x] REFACTOR: keep verify-specific assertions focused on public Go image behavior and cache boundaries, not whole-file snapshots

### Slice 5: Validation Lane Reuses Host-Side Rust Caches

- [x] RED: add one failing workflow contract that requires the validation lane to restore/save Cargo dependency and build caches instead of cold-starting on every run
- [x] GREEN: add the smallest truthful host-cache restore/save steps to `validate`
- [x] REFACTOR: keep the cache keys and paths behind `GithubWorkflowContract` helpers rather than scattering string literals through the test file

### Slice 6: Publish Lane Reuses Remote BuildKit Caches And Stops Serializing Three Independent Images

- [x] RED: add one failing workflow contract that requires:
  - explicit `--cache-from`
  - explicit `--cache-to`
  - per-image cache scopes from the shared build-target contract
  - one speed improvement beyond basic cache reuse by parallelizing independent image publication
- [x] GREEN: implement the smallest honest workflow topology that satisfies those constraints
  - preferred direction: matrix or separate per-image publish jobs
  - keep downstream manifest assembly after the parallel image jobs
- [x] REFACTOR: remove the old single-job sequential publish bottleneck rather than carrying both topologies

### Slice 7: Native `arm64` Evaluation And Decision Lock-In

- [x] RED: add one failing workflow/task-outcome contract that requires the implementation to state whether native `arm64` publication is adopted or rejected
- [x] GREEN: during execution, inspect the repo-owned runner configuration available to the workflow and make one honest choice:
  - adopt a trusted native `arm64` path only if a real trusted runner label/path exists and can be encoded/tested without guesswork
  - otherwise reject the native path and keep the cache-optimized emulated strategy explicit
- [x] REFACTOR: keep the chosen platform strategy encoded in one workflow boundary, not as comments and drift-prone scattered strings

### Slice 8: Cargo-Chef Decision And Final Contract Tightening

- [x] RED: add one failing Docker/workflow contract that requires the task outcome to reflect whether `cargo-chef` was adopted or rejected for the Rust image path
- [x] GREEN: complete the Rust implementation with `cargo-chef` if it is the honest speed win, or reject it explicitly if the tracer-bullet slices show it is not workable here
- [x] REFACTOR: remove any leftover duplicate Rust build plumbing that remains after the final decision

### Slice 9: Final Verification And Boundary Cleanup

- [x] RED: if any contract/doc/task result still allows the cache topology, arm64 decision, or cargo-chef decision to drift silently, add the smallest failing coverage needed
- [x] GREEN: complete the task acceptance updates only after all required lanes pass
- [x] REFACTOR: do one final `improve-code-boundaries` pass so:
  - build-target topology has one honest owner
  - shared Rust cache assertions have one honest owner
  - workflow cache behavior is asserted from one honest workflow contract

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long` only if execution changes the ultra-long lane or proves it is required
- [x] One final `improve-code-boundaries` pass after all required lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every required lane passes

Plan path: `.ralph/tasks/story-21-github-workflows-image-publish/02-task-massively-improve-image-build-speed-with-docker-layer-and-build-cache-reuse_plans/2026-04-20-image-build-cache-speed-plan.md`

NOW EXECUTE
