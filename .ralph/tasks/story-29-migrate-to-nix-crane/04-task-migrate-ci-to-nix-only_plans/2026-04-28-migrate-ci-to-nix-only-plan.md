# Plan: Migrate CI To Nix Only

## References

- Task:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/04-task-migrate-ci-to-nix-only.md`
- Prior story steps:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/02-task-migrate-build-run-test-lint-to-crane.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/02-task-migrate-build-run-test-lint-to-crane_plans/2026-04-28-migrate-build-run-test-lint-to-crane-plan.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/03-task-migrate-docker-image-generation-to-nix.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/03-task-migrate-docker-image-generation-to-nix_plans/2026-04-28-nix-image-generation-plan.md`
- Workflow surfaces in scope:
  - `.github/workflows/publish-images.yml`
  - `.github/workflows/promote-image-tags.yml`
  - `.github/workflows/image-catalog.yml`
  - `.github/workflows/AGENTS.md`
- Build and image surfaces:
  - `flake.nix`
  - `Makefile`
- Required skills:
  - `tdd`
  - `improve-code-boundaries`
  - `github-api-auth-wrapper`
- Smell set for this task:
  - `.ralph/tasks/smells/2026-04-28-story-29-ci-to-nix-boundaries.md`

## Planning Assumptions

- This turn started with no task-04 plan artifact, so this is a planning turn and must stop after the plan is written.
- Task 03 already moved runtime image generation into `flake.nix`.
  - Task 04 must make GitHub Actions consume those Nix image outputs rather than rebuilding via Dockerfiles.
- This is a workflow task, so TDD must use executable public behavior rather than fake file-content tests.
  - Do not add Rust tests that assert workflow strings.
  - Do not add shell tests that grep YAML.
  - Treat hosted workflow runs as the truthful public test for workflow behavior.
- Repo policy still requires final repo gates on the execution turn:
  - `make check`
  - `make lint`
  - `make test`
  - Those are regression gates for the repo state, not proof that the workflow logic itself works.
- `make test-long` is not an end-of-task default gate here.
  - The workflow may still expose or use a long lane if the story requires it, but the task-completion gate for this task remains the normal repo lanes plus hosted verification.
- No backwards compatibility is allowed.
  - Dockerfile-driven CI build paths should be removed rather than preserved behind fallback branches.
  - If a workflow helper exists only to preserve the old Dockerfile catalog boundary, delete it.
- Hosted verification must use `/home/joshazimullah.linux/github-api-curl` so the execution turn can inspect authenticated Actions runs and logs without exposing tokens.
- If execution proves the flake cannot honestly express the image publish metadata needed by both `publish-images` and `promote-image-tags`, switch this plan back to `TO BE VERIFIED` instead of hardcoding that metadata in two workflow files again.

## Current State Summary

- The flake already owns:
  - `check`
  - `lint`
  - `test`
  - `test-long`
  - `runner-image`
  - `verify-image`
- The current GitHub workflow still has a split control plane:
  - validation lanes install Rust manually and run `make`
  - image publication lanes install Docker Buildx and rebuild from Dockerfiles
  - manifest publication stitches those Docker-built per-arch images together
  - `image-catalog.yml` hand-emits string JSON that duplicates image identity already implied by `flake.nix`
- Hosted evidence gathered during planning:
  - latest `publish-images` run is run `#75` on commit `78060cb163138d27896e3e4e065c77a93ce40964`
  - that run started on `2026-04-27T23:55:42Z`
  - `validate-fast` failed
  - `validate-long` failed
  - all `publish-image` jobs failed in the `Publish image` step
  - `quay-security-gate` and `publish-manifest` were skipped downstream
  - latest `promote-image-tags` run is run `#1` on commit `5daf0fca70c75f5927d6df365f08586ed207da49`
  - that run started on `2026-04-20T16:34:02Z` and also failed
- The current publish credentials and destination selectors already in repo are:
  - GitHub variable: `QUAY_ORGANIZATION`
  - GitHub secrets: `QUAY_ROBOT_USERNAME`, `QUAY_ROBOT_PASSWORD`
  - GitHub builtin token: `GITHUB_TOKEN`
- The execution turn must preserve those names unless there is a compelling reason to change them and record the final exact names in workflow comments or task notes.

## Improve-Code-Boundaries Focus

- Primary boundary smell:
  - `flake.nix` owns the image build graph
  - `.github/workflows/image-catalog.yml` owns image identity and publish metadata
  - `.github/workflows/publish-images.yml` owns Dockerfile paths and rebuild strategy
  - `.github/workflows/promote-image-tags.yml` depends on that same workflow-owned catalog
- Desired boundary after execution:
  - `flake.nix` becomes the single source of truth for build, test, image, and publish metadata
  - workflow files become orchestration only:
    - install Nix
    - ask the flake what to build/publish
    - build the flake outputs
    - push those outputs
    - verify/publish manifests
- Secondary boundary smell:
  - validation jobs and publish jobs duplicate bootstrap and dependency installation logic
- Desired cleanup:
  - one honest Nix bootstrap pattern
  - one flake-derived catalog boundary
  - no Dockerfile/context/cache metadata left in workflow YAML
- Important non-goal:
  - do not replace the current duplication with a pile of one-off shell helper scripts.
  - only extract a helper if it is reused by multiple workflows and represents a real boundary.

## Proposed Public Workflow Interface

- Trigger surfaces:
  - `push` on `master` for the protected publish path
  - `workflow_dispatch` for safe hosted validation of the Nix-only workflow without requiring a release/publish event
- Protected publish behavior:
  - publish credentials are available only to the `push` on `master` path
  - pull requests, forks, and manual validation runs must not gain Quay publish credentials
- Nix-owned command surfaces that GitHub must call:
  - `nix run .#check`
  - `nix run .#lint`
  - `nix run .#test`
  - `nix build .#runner-image`
  - `nix build .#verify-image`
- Publish metadata boundary:
  - add one flake-evaluable structured output for the image catalog and platform matrix
  - the workflow should consume it via `nix eval --json ...`
  - the same flake output should serve both `publish-images` and `promote-image-tags`
- Registry outputs:
  - per-arch pushed image tags for both images
  - one multi-platform final tag per image keyed by commit SHA
  - GHCR copies or mirror tags remain derived from those already-built artifacts rather than rebuilt locally

## Type And Interface Decisions

- Prefer a single structured flake output such as:
  - `github.publishImageCatalog`
  - or an equivalently clear top-level attr
- That output should own:
  - image ids
  - flake package names for image archives
  - manifest keys
  - Quay repository names
  - GHCR repository names
  - artifact names
  - supported platform matrix
- It must not own:
  - secrets
  - branch protection policy
  - ephemeral run-specific tags like `github.sha`
- The workflow should derive runtime tags from `github.sha` and branch/event context, while all static image metadata comes from the flake.
- Prefer publishing the Nix-built Docker archives directly with `skopeo copy docker-archive:... docker://...` or an equally honest archive-to-registry path.
  - Do not `docker build`.
  - Do not `docker buildx build`.
- Keep manifest creation as a separate step/job that combines the previously published per-arch refs into one multi-platform tag.

## TDD Execution Strategy

- Hosted workflow behavior is the public interface, so execution must use vertical slices with real commands and hosted verification.
- Local commands are allowed only for the non-hosted surfaces the workflow depends on:
  - `nix eval`
  - `nix build`
  - `make check`
  - `make lint`
  - `make test`
- For each workflow slice:
  - RED:
    - identify one current failing public behavior in hosted Actions or a missing flake-evaluable surface
    - make that failure explicit through a real command or hosted run
  - GREEN:
    - add the minimal workflow/flake change to make only that behavior pass
  - REFACTOR:
    - remove duplicated YAML/bootstrap/catalog logic only after the current slice is green
- The execution turn must keep a record of:
  - the exact commit SHA pushed for hosted verification
  - the exact Actions run id(s) checked with `github-api-curl`
  - the exact job(s)/step(s) that passed or failed

## Vertical Execution Slices

### Slice 1: Flake-Owned Publish Catalog Tracer Bullet

- [ ] RED:
  - add one real `nix eval --json` call target for publish metadata and run it so it fails because the flake does not expose that structured catalog yet
- [ ] GREEN:
  - add the minimal flake output exposing image metadata and the platform matrix in JSON-friendly form
  - change one workflow surface to consume that flake-derived data instead of the heredoc catalog
- [ ] REFACTOR:
  - delete `.github/workflows/image-catalog.yml` if it no longer owns a truthful boundary
  - update `promote-image-tags.yml` to consume the same flake-derived catalog rather than a separate YAML-owned one
- Stop condition:
  - if the flake output shape becomes awkward enough that the workflow still needs to duplicate static image metadata, switch back to `TO BE VERIFIED`

### Slice 2: Validation Jobs Become Nix Bootstrap + Flake Commands

- [ ] RED:
  - make one hosted-safe validation lane fail on the absence of a Nix bootstrap path or safe trigger path
  - use `workflow_dispatch` or another non-publish path so the validation lane can be tested without Quay secrets
- [ ] GREEN:
  - install Nix in the workflow
  - replace manual Rust bootstrap with flake-native validation commands
  - run `check`, `lint`, and `test` through the flake-backed public interface
- [ ] REFACTOR:
  - collapse duplicated validate bootstrap logic into one honest pattern
  - remove Cargo cache/install logic that only existed because Rust was being installed outside Nix

### Slice 3: One Architecture Publish Lane From Nix-Built Image Archives

- [ ] RED:
  - move one publish path off `docker buildx build` and make it fail honestly until the job can publish a Nix-built image artifact
- [ ] GREEN:
  - build the image archive with `nix build .#runner-image` or `nix build .#verify-image`
  - publish that archive to the registry under an architecture-specific tag without rebuilding it
  - persist the exact published per-arch refs as artifacts for downstream jobs
- [ ] REFACTOR:
  - remove Dockerfile/context/cache-scope inputs from the publish matrix now that the build path is Nix-owned

### Slice 4: Parallelize Both Architectures And Both Images

- [ ] RED:
  - extend the first publish slice to a full matrix and let one real matrix axis fail if any remaining workflow assumptions are still Dockerfile-specific
- [ ] GREEN:
  - publish all required `(image, platform)` combinations in parallel using the matrix
  - keep `fail-fast: false` so one broken axis does not hide the others
- [ ] REFACTOR:
  - keep platform-specific logic data-driven from the flake catalog instead of embedding more branchy YAML

### Slice 5: Vulnerability Gate On Nix-Built Artifacts

- [ ] RED:
  - keep the security gate wired to the per-arch published refs and let it fail honestly if it still expects Dockerfile-built artifacts or missing artifact names
- [ ] GREEN:
  - preserve the existing Quay security evaluation or replace it with Trivy only if that is the already-supported equivalent and the task scope requires it
  - ensure the gate runs against the Nix-published refs before manifest publication
- [ ] REFACTOR:
  - remove any old security-gate assumptions tied to Docker build outputs rather than published image refs

### Slice 6: Multi-Platform Manifest Publication From Existing Artifacts

- [ ] RED:
  - make the manifest publish job fail honestly if any image lacks both architecture refs or if it still expects legacy catalog keys
- [ ] GREEN:
  - compose one multi-platform tag per image from the already-published per-arch refs
  - mirror or copy the final manifest/tag to GHCR from the published source image rather than rebuilding
- [ ] REFACTOR:
  - keep manifest-key ownership in the flake catalog so `publish-images` and `promote-image-tags` stay aligned

### Slice 7: Protected-Credential Scoping And Hosted Proof

- [ ] RED:
  - audit the workflow conditions so publish secrets would be unavailable on manual validation, PRs, and forks; fail the slice if the job graph still exposes publish steps outside `push` on `master`
- [ ] GREEN:
  - scope Quay and GHCR publish/login steps to the protected branch/event path only
  - add or update workflow comments/task notes to document the exact names:
    - `vars.QUAY_ORGANIZATION`
    - `secrets.QUAY_ROBOT_USERNAME`
    - `secrets.QUAY_ROBOT_PASSWORD`
    - `secrets.GITHUB_TOKEN`
- [ ] REFACTOR:
  - delete any leftover branch/event conditions that exist only to support the removed Dockerfile workflow shape
- [ ] Hosted verification:
  - push the execution commit
  - inspect the resulting Actions run(s) with `/home/joshazimullah.linux/github-api-curl`
  - if the run still fails on a real unresolved issue, capture that exact issue in a follow-up task or bug instead of masking it

## Expected File Shape After Execution

- `flake.nix`
  - gains one structured publish-catalog output consumable by workflows
- `.github/workflows/publish-images.yml`
  - becomes Nix-native for validation, build, publish, and manifest assembly
- `.github/workflows/promote-image-tags.yml`
  - consumes the same flake-owned catalog metadata rather than the old YAML catalog helper
- `.github/workflows/image-catalog.yml`
  - should be deleted unless execution proves it still owns a real reusable boundary after static catalog data moves into the flake
- Optional helper extraction:
  - only if there is a genuinely reusable bootstrap boundary shared by multiple workflows
  - otherwise keep orchestration in the workflow and keep static metadata in the flake

## Final Validation Requirements For Execution Turn

- [ ] Run repo regression gates:
  - `make check`
  - `make lint`
  - `make test`
- [ ] Do not run `make test-long` as the default task-end gate.
- [ ] Perform hosted verification against the actual GitHub Actions run created by the execution commit.
- [ ] Record in task notes or workflow comments:
  - the exact publish variable/secret names
  - the exact hosted run id
  - the exact validation commands and Nix outputs used
- [ ] Do one final `improve-code-boundaries` pass:
  - no Dockerfile build knowledge remains in workflow metadata
  - no second image catalog exists outside the flake
  - no hidden `docker build` or ignored shell errors remain

## Expected Outcome

- GitHub Actions stops rebuilding images through Dockerfiles.
- The flake becomes the single source of truth for validation, image build, and static publish metadata.
- Per-architecture images are published from Nix-built archives in parallel and then combined into one multi-platform tag per image.
- Publish credentials stay scoped to the protected publish path.
- Hosted verification uses authenticated Actions evidence rather than fake local workflow tests.

Plan path: `.ralph/tasks/story-29-migrate-to-nix-crane/04-task-migrate-ci-to-nix-only_plans/2026-04-28-migrate-ci-to-nix-only-plan.md`

NOW EXECUTE
