# Plan: Remove Novice-User Dependence On Repo Clone And Local Tooling

## References

- Task: `.ralph/tasks/story-16-runtime-split-removals/04-task-remove-novice-user-dependence-on-repo-clone-and-local-tooling.md`
- Related prior plans:
  - `.ralph/tasks/story-16-runtime-split-removals/03-task-remove-bash-bootstrap-flows-and-script-based-source-setup_plans/2026-04-19-sql-only-source-setup-plan.md`
  - `.ralph/tasks/story-15-ci-build-test-image-pipeline/01-task-build-master-only-pipeline-for-full-tests-and-scratch-ghcr-image_plans/2026-04-19-master-only-ghcr-scratch-pipeline-plan.md`
  - `.ralph/tasks/story-13-verify-novice-user/02-task-verify-direct-docker-build-and-run-without-wrapper-scripts_plans/2026-04-19-direct-docker-build-run-plan.md`
- Current operator docs:
  - `README.md`
- Current image publish surface:
  - `.github/workflows/master-image.yml`
  - `crates/runner/tests/ci_contract.rs`
  - `crates/runner/tests/support/github_workflow_contract.rs`
- Current runner image contract:
  - `Dockerfile`
  - `crates/runner/tests/readme_contract.rs`
  - `crates/runner/tests/support/runner_docker_contract.rs`
  - `crates/runner/tests/support/runner_image_harness.rs`
- Current source-bootstrap contract:
  - `crates/source-bootstrap/src/lib.rs`
  - `crates/source-bootstrap/tests/cli_contract.rs`
  - `crates/source-bootstrap/tests/bootstrap_contract.rs`
  - `crates/source-bootstrap/tests/support/readme_contract.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown is treated as approval for the public interface and behavior priorities in this turn.
- The novice-user contract now requires published images only:
  - no `cargo run`
  - no local `docker build`
  - no repo checkout as part of the supported operator path
- Source bootstrap is still a supported operator action, so it cannot remain tied to a local Rust toolchain or repo-local binary invocation.
- Contributor-only local image builds may remain inside internal test harnesses where they are needed to validate the repo, but those assumptions must not leak into README-owned operator docs or public-contract tests.
- The existing publish workflow currently ships only the runner image. Task 04 therefore needs a real published source-bootstrap image path, not just README prose that names a non-existent image.
- The public novice path should pin to an explicit published image tag variable such as a validated commit SHA, not an implicit `latest` contract.
- If execution shows that the source-bootstrap image cannot be added cleanly without a broader packaging redesign than this task can justify, switch this plan back to `TO BE VERIFIED` immediately and stop.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - the source bootstrap path works through a containerized `source-bootstrap` binary, not `cargo run`
  - the README quick starts begin from pulling published images only
  - CI publishes both required novice-path images under the trusted master-only gate
  - public-contract tests fail if repo-local build or local-tooling assumptions return to the novice path
- Lower-priority concerns:
  - reshaping contributor-only local test harnesses that are not part of the public operator contract
  - general README polish beyond the image-only novice path

## Problem To Fix

- `README.md` still tells the operator to run `cargo run -p source-bootstrap -- ...` for source bootstrap and `docker build -t cockroach-migrate-runner .` for the destination runtime.
- `.github/workflows/master-image.yml` currently publishes only `ghcr.io/<owner>/cockroach-migrate-runner:${github.sha}`, so the source bootstrap path has no corresponding published-image contract.
- `crates/runner/tests/support/runner_docker_contract.rs` currently mixes two unrelated responsibilities:
  - public README assertions about the operator contract
  - local docker command assembly for contributor-side harnesses
- The current fast tests protect a SQL-only source bootstrap contract, but they do not protect an image-only novice-user contract.

## Interface And Boundary Decisions

- The public novice-user contract starts from published image coordinates and an explicit image tag variable.
  - README should show `docker pull` plus `docker run` against published image names
  - README should not rely on the repo root `Dockerfile` as part of the operator path
- Introduce a dedicated source-bootstrap image boundary.
  - preferred ownership: `crates/source-bootstrap/Dockerfile`
  - build context may remain the repository root for CI and test builds if needed, but the Dockerfile should live with the source-bootstrap slice rather than being hidden inside runner-owned files
  - runtime image should start the `source-bootstrap` binary directly with a JSON entrypoint and no shell wrapper
- Keep the existing runner image contract direct and scratch-based, but change the public docs from local build instructions to published-image instructions.
- Publish both novice-user images under the trusted workflow:
  - runner image
  - source-bootstrap image
- Separate public published-image contract logic from local docker harness plumbing.
  - README/public assertions should move toward one honest "published image contract" support boundary
  - local build/test command assembly should stay with harness files that actually own contributor-side docker execution

## Improve-Code-Boundaries Focus

- Primary smell: wrong-place public contract logic in `crates/runner/tests/support/runner_docker_contract.rs`.
  - it currently owns both operator-facing README expectations and contributor-local build command assembly
  - split those responsibilities so public image coordinates and README guarantees no longer share a helper with local image-build plumbing
- Secondary smell: mixed image-coordinate knowledge across README text, CI workflow env, and test assertions.
  - execution should reduce this to one clear boundary in tests and one clear shared boundary in workflow env
- Tertiary smell: source-bootstrap has a public CLI contract but no matching image boundary even though the novice path now requires one.
  - add the image boundary directly rather than teaching tests to bless `cargo run`

## Public Contract To Establish

- `README.md` documents the novice path with published image coordinates only.
  - source bootstrap uses `docker run --rm ... <published-source-bootstrap-image> render-bootstrap-sql --config ...`
  - destination runtime uses `docker pull` plus `docker run --rm ... <published-runner-image> ...`
- `README.md` does not contain:
  - `cargo run -p source-bootstrap`
  - `docker build -t cockroach-migrate-runner .`
  - instructions to clone the repo as part of the operator path
- A real `source-bootstrap` image exists, builds in tests, and exposes only the source bootstrap command surface directly from the binary entrypoint.
- The publish workflow validates the repo, then builds and publishes both novice-path images under the same trusted master-push gate and explicit registry boundaries.
- Public-contract tests fail loudly if repo-local build-from-source steps or local tooling assumptions re-enter the novice-user path.

## Files And Structure To Add Or Change

- [x] `README.md`
  - rewrite source bootstrap and Docker quick starts around `docker pull` and published image references only
- [x] `.github/workflows/master-image.yml`
  - extend the trusted publish workflow so it ships both runner and source-bootstrap images
- [x] `crates/source-bootstrap/Dockerfile`
  - add a dedicated image build for the source-bootstrap binary with a direct binary entrypoint
- [x] `crates/source-bootstrap/tests/image_contract.rs`
  - add a fast public image-contract test suite for source-bootstrap
- [x] `crates/source-bootstrap/tests/support/source_bootstrap_image_contract.rs`
  - preferred support owner for source-bootstrap image assertions
- [x] `crates/source-bootstrap/tests/support/source_bootstrap_image_harness.rs`
  - only if execution needs a reusable local docker harness for image build/run assertions
- [x] `crates/source-bootstrap/tests/bootstrap_contract.rs`
  - strengthen README/config contract coverage so the novice path is protected through the image surface where appropriate
- [x] `crates/source-bootstrap/tests/cli_contract.rs`
  - reviewed; existing help contract already matched the published image entrypoint surface without a source change
- [x] `crates/runner/tests/readme_contract.rs`
  - replace local-build README assumptions with published-image novice-path assertions
- [x] `crates/runner/tests/support/runner_docker_contract.rs`
  - reduce it to runner-image-specific contract helpers or split public contract knowledge out entirely
- [x] `crates/runner/tests/ci_contract.rs`
  - require both published novice-user images and the unchanged trust gates
- [x] `crates/runner/tests/support/github_workflow_contract.rs`
  - extend workflow parsing/assertions for dual-image publish and shared registry boundaries
- [x] Add or rename one shared public-contract support file if needed
  - only if execution needs one typed owner for published image references across README and workflow assertions

## TDD Execution Order

### Slice 1: Tracer Bullet For A Real Source-Bootstrap Image

- [x] RED: add one failing public image-contract test that requires a dedicated source-bootstrap Dockerfile, a direct binary entrypoint, and a successful containerized `render-bootstrap-sql` path
- [x] GREEN: add the smallest source-bootstrap image implementation needed for that test to pass
- [x] REFACTOR: keep the Dockerfile and image contract under the source-bootstrap-owned boundary instead of teaching runner-side helpers about it

### Slice 2: Move The README Novice Path To Published Images Only

- [x] RED: add failing README contract coverage that rejects `cargo run -p source-bootstrap`, rejects local `docker build` in the novice path, and requires explicit published-image pull/run steps
- [x] GREEN: rewrite the README quick starts around published image coordinates and an explicit image tag variable
- [x] REFACTOR: pull published-image README assertions out of `runner_docker_contract.rs` so public docs are not coupled to local harness command assembly

### Slice 3: Publish Both Required Images Under The Trusted Workflow

- [x] RED: extend CI contract coverage so the workflow fails unless it publishes both the runner image and the source-bootstrap image through shared trusted registry boundaries
- [x] GREEN: update `.github/workflows/master-image.yml` to build, scan, and publish both images without weakening the existing trust gate
- [x] REFACTOR: centralize image repository env handling and workflow test assertions so image coordinates do not drift across duplicated literals

### Slice 4: Protect The README Fixture And Public Surface Through The Image Boundary

- [x] RED: add the next failing source-bootstrap contract that proves the README-owned config example works through the image surface rather than a local cargo binary path
- [x] GREEN: make the minimum harness or contract changes needed so the README fixture stays copyable through the image entrypoint
- [x] REFACTOR: delete or flatten any test support that still pretends the novice contract is a local binary invocation when the real public surface is now the image

### Slice 5: Remove Remaining Novice-Path Build-From-Source Leakage

- [x] RED: add one final failing public-contract assertion where needed so lingering novice-path references to repo checkout, local tool installs, or local Docker builds fail loudly
- [x] GREEN: remove the remaining public-surface leakage from docs, tests, examples, or messages without touching contributor-only private harnesses unnecessarily
- [x] REFACTOR: confirm the remaining public contract helpers describe published images only, while contributor-local harness logic stays in its own boundary

### Slice 6: Repository Lanes

- [x] RED: run `make check`, `make lint`, and `make test`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required default lane passes cleanly
- [x] REFACTOR: run `make test-long` only if execution changes ignored long tests or their selection; otherwise skip that lane for this task and do one final `improve-code-boundaries` pass

## TDD Guardrails For Execution

- Every new assertion must fail before the supporting image, workflow, or README change is added.
- Do not satisfy this task with README prose alone. The published novice path must be backed by a real source-bootstrap image contract and a real publish workflow.
- Do not keep `cargo run` or local `docker build` as undocumented fallback paths in the README. No backwards compatibility is allowed for the novice contract.
- Do not weaken the existing master-only publish safety model in order to add the second image.
- Do not introduce `latest` tags or ambiguous image references into the public contract.
- Do not swallow docker build, docker run, or workflow-contract failures. They must fail loudly with concrete messages.

## Boundary Review Checklist

- [x] Public README assertions and local docker harness plumbing no longer share one mixed helper
- [x] The source-bootstrap image boundary is owned by the source-bootstrap slice
- [x] The novice-user contract starts from published image references only
- [x] Workflow env and tests have one clear image-coordinate boundary
- [x] No repo-clone or local-tooling assumption survives in the public novice path
- [x] No error path is swallowed

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long` only if execution changes ignored long tests or their selection; this task did not change the long-lane selection boundary
- [x] One final `improve-code-boundaries` pass after the required lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every required lane passes

Plan path: `.ralph/tasks/story-16-runtime-split-removals/04-task-remove-novice-user-dependence-on-repo-clone-and-local-tooling_plans/2026-04-19-published-images-only-novice-path-plan.md`

NOW EXECUTE
