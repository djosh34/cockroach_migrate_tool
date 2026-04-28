## Task: Create Magic Nix Cache Matrix Workflow And Combine Image Artifacts <status>completed</status> <passes>true</passes>

<description>
**Goal:** Create a GitHub Actions workflow that runs five jobs in parallel, uses Magic Nix Cache everywhere Nix runs, builds per-architecture image artifacts for both `runner-image` and `verify-image`, runs `nix flake check` at the same time, then combines and publishes the per-architecture artifacts as exactly one multi-platform GHCR `runner-image` tag and one multi-platform GHCR `verify-image` tag after all five parallel jobs pass. The higher order goal is to make hosted CI fast, observable, and reproducible while publishing commit-SHA-tagged multi-platform images to GHCR without rebuilding images in the final assembly/publish step.

This is a workflow/infrastructure task, not an application-code task. TDD is not allowed for this task. Do not use Rust test assertions for workflow text. Verification must be manual and must prove the real GitHub workflow behavior through local syntax checks and authenticated hosted workflow logs.

In scope:
- Add or replace the relevant GitHub Actions workflow for the image pipeline.
- Use the latest documented Magic Nix Cache action form. As of task creation on 2026-04-28, the GitHub Marketplace and Determinate Systems examples show `DeterminateSystems/magic-nix-cache-action@main` as the latest-version usage.
- Install/configure Nix in every job that runs Nix and enable Magic Nix Cache in every such job so all Nix jobs can reuse shared cache entries across workflow runs and across jobs where GitHub cache semantics allow it.
- Run exactly these five independent jobs in parallel before image assembly:
  - `runner-image` for `amd64`.
  - `runner-image` for `arm64`.
  - `verify-image` for `amd64`.
  - `verify-image` for `arm64`.
  - `nix flake check`.
- Implement the four image builds using a matrix over image name and architecture, or an equivalently clear matrix that produces those four parallel runs.
- Make `nix flake check` run concurrently with the matrix image builds, not after them.
- Run all Nix commands in a log-readable way, including flags and shell settings that preserve failure status and surface build output in the GitHub job logs. The implementer should prefer commands such as `nix flake check --print-build-logs --show-trace` and `nix build ... --print-build-logs --show-trace`, with `set -euo pipefail`.
- Upload the four per-architecture image artifacts from the matrix jobs in a form that can be combined without rebuilding. The final assembly job must consume those artifacts directly.
- After all five jobs pass, run one dependent assembly/publish job that combines the two architecture-specific `runner-image` artifacts into one multi-platform image tag and combines the two architecture-specific `verify-image` artifacts into one multi-platform image tag.
- Publish the final multi-platform `runner-image` and `verify-image` tags to GHCR from the assembly/publish job.
- Use least-privilege GitHub Actions permissions: only the assembly/publish job should have `packages: write`, and non-publish jobs should not inherit package write permission.
- Authenticate to GHCR using the repository-supported GitHub Actions identity or configured repository secrets, without printing credentials or tokens in logs.
- The final multi-platform tags must use the exact Git commit SHA only. Do not append `-arm64`, `-amd64`, `-x86_64`, architecture suffixes, branch names, dates, or mutable aliases to the final tags.
- Do not build the final multi-platform images with Nix. Nix may produce the per-architecture image artifacts, but final multi-platform assembly must combine those artifacts using image tooling such as `docker buildx imagetools`, `skopeo`, `oras`, `crane`, or another OCI-aware tool.
- Keep the workflow observable enough that a maintainer can read the GitHub pipeline logs and understand which Nix command ran, which image/architecture was built, which artifact was uploaded, how the final multi-platform tags were assembled, and which GHCR image references/digests were published.
- Do not add a GitHub Actions `concurrency` attribute anywhere in this workflow. Every push must start its own full workflow run, and old runs must not be cancelled or superseded by newer pushes. If an older run is still running, the newer run must run at the same time.
- Ensure failures are not swallowed. Every command that can fail must fail the job. Any unavoidable warning or non-fatal condition must be explicitly justified in task notes, and any ignored real error must be filed as a bug task instead of hidden.

Out of scope:
- Rewriting Nix package/image definitions except where required to expose per-architecture image artifacts cleanly to the workflow.
- Reintroducing Dockerfile-based image builds.
- Running Cargo directly. This repo requires Nix, never `cargo`, for build/test/lint execution.
- Creating architecture-suffixed final image tags.
- Publishing unrelated images, publishing to registries other than GHCR, or restoring legacy CI behavior.
- Publishing mutable tags such as `latest`, branch names, release aliases, or architecture-specific tags.
- Adding workflow `concurrency`, `cancel-in-progress`, superseding behavior, queue-collapsing behavior, or any other mechanism that cancels/skips older runs when newer pushes arrive.
- Skipping tests, lint, or workflow verification because the task is "only CI".

Decisions already made:
- Magic Nix Cache is required and must be used in all Nix jobs.
- The five expensive/validation jobs must be parallel: four image builds plus `nix flake check`.
- The workflow must wait for all five jobs to pass before creating the final multi-platform image tags.
- The workflow must publish the final multi-platform images to GHCR after assembly.
- The final tags must be the git commit SHA only.
- Per-architecture artifacts may have internal artifact names that include architecture, but user-facing final image tags must not.
- Multi-platform assembly must combine existing per-architecture artifacts and must not trigger a fresh Nix build.
- GHCR publish permissions must be restricted to the assembly/publish job.
- The workflow must not use GitHub Actions `concurrency`; repeated pushes should create independent workflow runs that can execute concurrently.
- Pipeline logs must show useful Nix build/check output instead of hiding it behind quiet wrappers.

</description>


<acceptance_criteria>
- [x] GitHub Actions contains a workflow for the image pipeline using Magic Nix Cache in every job that invokes Nix.
- [x] The workflow uses the latest documented Magic Nix Cache action usage, currently `DeterminateSystems/magic-nix-cache-action@main` as of 2026-04-28.
- [x] The workflow has four parallel image-build runs covering `runner-image` on `amd64`, `runner-image` on `arm64`, `verify-image` on `amd64`, and `verify-image` on `arm64`.
- [x] `nix flake check` runs as a fifth parallel job at the same time as the four image-build runs.
- [x] All Nix invocations use log-readable failure-preserving commands, including `--print-build-logs` and `--show-trace` where applicable, and shell settings such as `set -euo pipefail`.
- [x] Each per-architecture image build uploads a non-empty artifact that the final assembly job consumes directly.
- [x] The final assembly job depends on all five parallel jobs and does not start unless all five succeeded.
- [x] The final assembly job creates exactly one multi-platform `runner-image` tag and exactly one multi-platform `verify-image` tag from the per-architecture artifacts.
- [x] The final multi-platform tags are exactly the git commit SHA, with no architecture suffixes or mutable aliases.
- [x] The final assembly step does not call Nix to build images and does not rebuild either image; it only combines already-built per-architecture artifacts.
- [x] The final assembly/publish job publishes the multi-platform `runner-image` and `verify-image` tags to GHCR.
- [x] Only the final assembly/publish job has GHCR package write permission; the Nix check and image-build jobs do not have `packages: write`.
- [x] GHCR authentication does not print credentials, tokens, or secret values in workflow logs.
- [x] The published GHCR image references use only the exact git commit SHA tag for both images.
- [x] The workflow verifies or records the published GHCR digests for both multi-platform images after push.
- [x] The workflow has no `concurrency` attribute and no cancellation/superseding behavior; every push creates a full run even when a previous run is still active.
- [x] The hosted GitHub workflow logs clearly show the Nix commands, image names, architectures, artifact upload/download names, multi-platform assembly commands, GHCR publish commands, and final published image references/digests.
- [x] Manual workflow syntax verification passes locally or through a workflow linter, and the exact command is recorded in task notes.
- [x] Manual hosted verification: trigger the workflow on GitHub, inspect authenticated workflow logs with `/home/joshazimullah.linux/github-api-curl` or an equivalent authenticated API path, and record evidence that the five jobs ran in parallel and the assembly/publish job waited for them.
- [x] Manual hosted publish verification: inspect GHCR or authenticated workflow output and record evidence that the commit-SHA-only multi-platform `runner-image` and `verify-image` tags were published successfully.
- [x] Manual hosted cache verification: record evidence from hosted workflow logs that Magic Nix Cache is active and reused across runs/jobs where available.
- [x] `make check` — passes cleanly unless the workflow-only nature of the change makes it inapplicable; if inapplicable, record the exact reason in task notes.
- [x] `make lint` — passes cleanly unless the workflow-only nature of the change makes it inapplicable; if inapplicable, record the exact reason in task notes.
</acceptance_criteria>

<plan>.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/01-task-create-magic-nix-cache-matrix-workflow-and-combine-image-artifacts_plans/2026-04-28-magic-nix-cache-matrix-ghcr-plan.md</plan>

<notes>
- Local validation commands:
  - `nix shell nixpkgs#actionlint -c actionlint .github/workflows/publish-images.yml`
  - `nix run .#test-molt`
  - `nix flake check --print-build-logs --show-trace`
  - `make check`
  - `make lint`
  - `make test`
- Hosted verification used `/home/joshazimullah.linux/github-api-curl`.
  - First push run `25068613846` proved the five-job parallel topology and artifact handoff, then failed honestly in `nix flake check` on `checks.x86_64-linux.molt-go-test` because the Cockroach runtime boundary still relied on a `buildFHSEnv` bubblewrap wrapper that GitHub's x86_64 runner rejected with `bwrap: setting up uid map: Permission denied`.
  - The fix flattened that boundary in `flake.nix` to a direct autopatched Cockroach binary package, which kept `nix flake check` real instead of skipping `test-molt`.
  - Final hosted success run `25069126637`: the five prerequisite jobs started at `2026-04-28T17:55:44Z` or `2026-04-28T17:55:45Z`, proving the four matrix builds and `nix flake check` ran in parallel.
  - The publish job in run `25069126637` started at `2026-04-28T18:05:00Z`, after the last prerequisite (`nix flake check`) completed at `2026-04-28T18:04:56Z`.
  - Hosted cache evidence from run `25069126637`:
    - the Magic Nix Cache step logged `Native GitHub Action cache is enabled.`
    - the arm64 verify-image build copied `/nix/store/4lpg5gbm9qlj30f5qd115kp2p6bd4hqi-docker-image-verify-image.tar.gz` from `http://127.0.0.1:37515`, showing reuse from the Magic Nix Cache daemon instead of rebuilding the image archive
  - Hosted flake-check evidence from run `25069126637`:
    - `checks.x86_64-linux.test-molt` and `checks.x86_64-linux.molt-go-test` both finished green
    - the earlier `bwrap` failure disappeared after the Cockroach runtime refactor
  - Hosted publish evidence from run `25069126637`:
    - final `runner-image` ref: `ghcr.io/djosh34/runner-image:df3435b59b8c8f5987d0c90b125ecbea389764ef`
    - final `verify-image` ref: `ghcr.io/djosh34/verify-image:df3435b59b8c8f5987d0c90b125ecbea389764ef`
    - final runner-image manifest digest: `sha256:9ad926685a94c276bd6d8bd657ad0ecf8a3cae489b475e6071e1d24770c0fee5`
    - final verify-image manifest digest: `sha256:f2704eaeeab12de93b35742b43130dbb795abc06c87fe2db0c1b858673762c9a`
    - publish assembly used downloaded artifacts plus temporary per-architecture refs:
      - `ghcr.io/djosh34/runner-image:tmp-df3435b59b8c8f5987d0c90b125ecbea389764ef-25069126637-amd64`
      - `ghcr.io/djosh34/runner-image:tmp-df3435b59b8c8f5987d0c90b125ecbea389764ef-25069126637-arm64`
      - `ghcr.io/djosh34/verify-image:tmp-df3435b59b8c8f5987d0c90b125ecbea389764ef-25069126637-amd64`
      - `ghcr.io/djosh34/verify-image:tmp-df3435b59b8c8f5987d0c90b125ecbea389764ef-25069126637-arm64`
- Non-fatal hosted warning that remains visible:
  - Determinate's installer emitted `FlakeHub Login failure: The process '/usr/local/bin/determinate-nixd' failed with exit code 1` before the Magic Nix Cache step.
  - The workflow does not swallow this condition; it is recorded here because the subsequent Magic Nix Cache logs explicitly show `use-flakehub: false`, `Disabling FlakeHub cache.`, and `FlakeHub cache is disabled.`
</notes>
