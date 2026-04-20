## Task: Massively improve image build speed with Docker layer reuse and shared Rust/Go dependency caches <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Make the GitHub image workflow materially faster by redesigning the Dockerfiles, workflow cache strategy, and build topology so repeated validation, test, and publish work reuses dependency downloads and intermediate compilation instead of starting cold on every lane. This task now precedes the main workflow-fix task inside the GitHub workflow story and explicitly unblocks it. The higher order goal is to turn image CI from the current 30-plus-minute wall-clock pain into a fast, repeatable pipeline that still proves the real product path while avoiding wasteful rebuilds across Rust, Go, multi-image stages, and cross-architecture image publishing.

In scope:
- improve Dockerfile caching strategy for the Rust image builds
- improve Dockerfile caching strategy for the Go verify image build
- explicitly evaluate a dedicated native `arm64` runner path for image builds so `arm64` artifacts do not depend on slow emulation if a trusted native runner can cut wall-clock time substantially
- separate dependency download, dependency resolution, and heavier compile layers so they survive unrelated source changes when possible
- partial-compile optimization for the Rust side so unchanged dependency graphs do not force full rebuild cost on every workflow run
- explicit evaluation of build-speed techniques such as `cargo-chef`, BuildKit cache mounts, dependency-only layers, and any other practical Rust/Go image-build acceleration methods discovered during implementation
- cache reuse across validation, test, image-build, and publish lanes rather than maintaining isolated cold-start paths for each job if the trust model allows reuse
- reuse of Docker/build caches across the runner, verify, and SQL-emitter image flow wherever shared inputs make that possible
- explicit evaluation of broader pipeline-speed wins beyond caching, including native-vs-emulated multi-arch strategy, job parallelization, stage reordering, artifact handoff, and other practical ways to drastically reduce total workflow runtime without weakening gates
- use of first-party or direct-install mechanisms for cache plumbing where practical rather than importing random third-party actions
- preserving correctness checks while making the pipeline faster
- documenting and testing the intended cache boundaries so accidental Dockerfile edits do not silently destroy cache effectiveness

Out of scope:
- weakening validation, test, or publish requirements just to get faster runtimes
- introducing hidden fallback paths that swallow cache or build errors
- unrelated runtime-feature work inside the images

Decisions already made:
- this task now takes precedence within the GitHub image workflow story and must pass before `.ralph/tasks/story-21-github-workflows-image-publish/01-task-fix-github-workflows-to-build-test-and-publish-the-three-image-split.md` resumes
- the objective is not a small polish pass; the workflow should be made massively faster where real cache reuse is available
- both Rust and Go image builds need attention
- native `arm64` build capacity is on the table if it materially beats emulated multi-arch builds and fits the repository trust/security model
- Rust-side work should explicitly consider tools such as `cargo-chef` for dependency planning and partial compilation reuse
- cache strategy should let the same warmed dependency/build state help `validate`, `test`, `build`, and publish-oriented lanes instead of rebuilding the same world repeatedly
- Dockerfile structure is part of the solution, not just the outer GitHub Actions YAML
- dependency downloads, module checks, and intermediate compile artifacts should be reused aggressively when inputs have not changed
- the task should attack end-to-end wall-clock time, not only isolated Docker build timings
- correctness remains mandatory; the task is only successful if the faster path still passes the required repository gates

</description>

<outcome>
- Adopted `cargo-chef` plus BuildKit cache mounts for the two Rust workspace image Dockerfiles so dependency planning, dependency cooking, and final binary builds stay in separate cache boundaries.
- Split the verify Go image Dockerfile into `go.mod`/`go.sum` dependency resolution and later source-copy/build steps with explicit Go module/build cache mounts.
- Added host-side Cargo registry and target cache restore/save steps to the validation lane so `make check` and `make test` stop cold-starting on every master push.
- Replaced the old single sequential publish bottleneck with a parallel `publish-image` matrix and a downstream `publish-manifest` aggregation job.
- Explicitly rejected a native `arm64` runner path for now because no trusted native `arm64` runner label is configured in repo-owned workflow configuration; the workflow keeps the emulated Buildx/QEMU path explicit instead of pretending a native runner exists.
- Added workflow and Dockerfile contract coverage for cache scopes, cache-friendly layer ordering, manifest aggregation, and the explicit `arm64` strategy decision so future edits fail loudly if they erode the speedup boundaries.
</outcome>


<acceptance_criteria>
- [x] Red/green TDD covers the intended cache-aware workflow and Dockerfile behavior for the image pipeline
- [x] Rust image builds use a cache-friendly structure that preserves dependency download and meaningful intermediate compile reuse across unrelated source edits where practical
- [x] Go image builds use a cache-friendly structure that preserves module download and build reuse across unrelated source edits where practical
- [x] The implementation explicitly evaluates and either adopts or rejects a dedicated native `arm64` runner/build path for `arm64` image publication, with the decision reflected in the task outcome
- [x] The implementation explicitly evaluates and adopts concrete Rust build-speed techniques through `cargo-chef`, with the choice reflected in the task outcome
- [x] Validation, test, build, and publish lanes reuse compatible caches instead of repeating avoidable cold-start dependency and compile work
- [x] The final design includes at least one concrete wall-clock pipeline-speed improvement beyond basic cache reuse through parallel image publication
- [x] Workflow/Dockerfile contract tests fail loudly if later edits would break the expected cache boundaries or reuse model
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] Ultra-long test selection was unchanged, so `make test-long` was not required for this task
</acceptance_criteria>

<plan>.ralph/tasks/story-21-github-workflows-image-publish/02-task-massively-improve-image-build-speed-with-docker-layer-and-build-cache-reuse_plans/2026-04-20-image-build-cache-speed-plan.md</plan>
