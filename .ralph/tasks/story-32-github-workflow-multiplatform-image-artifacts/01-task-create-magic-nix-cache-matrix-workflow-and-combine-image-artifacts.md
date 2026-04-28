## Task: Create Magic Nix Cache Matrix Workflow And Combine Image Artifacts <status>not_started</status> <passes>false</passes>

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
- [ ] GitHub Actions contains a workflow for the image pipeline using Magic Nix Cache in every job that invokes Nix.
- [ ] The workflow uses the latest documented Magic Nix Cache action usage, currently `DeterminateSystems/magic-nix-cache-action@main` as of 2026-04-28.
- [ ] The workflow has four parallel image-build runs covering `runner-image` on `amd64`, `runner-image` on `arm64`, `verify-image` on `amd64`, and `verify-image` on `arm64`.
- [ ] `nix flake check` runs as a fifth parallel job at the same time as the four image-build runs.
- [ ] All Nix invocations use log-readable failure-preserving commands, including `--print-build-logs` and `--show-trace` where applicable, and shell settings such as `set -euo pipefail`.
- [ ] Each per-architecture image build uploads a non-empty artifact that the final assembly job consumes directly.
- [ ] The final assembly job depends on all five parallel jobs and does not start unless all five succeeded.
- [ ] The final assembly job creates exactly one multi-platform `runner-image` tag and exactly one multi-platform `verify-image` tag from the per-architecture artifacts.
- [ ] The final multi-platform tags are exactly the git commit SHA, with no architecture suffixes or mutable aliases.
- [ ] The final assembly step does not call Nix to build images and does not rebuild either image; it only combines already-built per-architecture artifacts.
- [ ] The final assembly/publish job publishes the multi-platform `runner-image` and `verify-image` tags to GHCR.
- [ ] Only the final assembly/publish job has GHCR package write permission; the Nix check and image-build jobs do not have `packages: write`.
- [ ] GHCR authentication does not print credentials, tokens, or secret values in workflow logs.
- [ ] The published GHCR image references use only the exact git commit SHA tag for both images.
- [ ] The workflow verifies or records the published GHCR digests for both multi-platform images after push.
- [ ] The workflow has no `concurrency` attribute and no cancellation/superseding behavior; every push creates a full run even when a previous run is still active.
- [ ] The hosted GitHub workflow logs clearly show the Nix commands, image names, architectures, artifact upload/download names, multi-platform assembly commands, GHCR publish commands, and final published image references/digests.
- [ ] Manual workflow syntax verification passes locally or through a workflow linter, and the exact command is recorded in task notes.
- [ ] Manual hosted verification: trigger the workflow on GitHub, inspect authenticated workflow logs with `/home/joshazimullah.linux/github-api-curl` or an equivalent authenticated API path, and record evidence that the five jobs ran in parallel and the assembly/publish job waited for them.
- [ ] Manual hosted publish verification: inspect GHCR or authenticated workflow output and record evidence that the commit-SHA-only multi-platform `runner-image` and `verify-image` tags were published successfully.
- [ ] Manual hosted cache verification: record evidence from hosted workflow logs that Magic Nix Cache is active and reused across runs/jobs where available.
- [ ] `make check` — passes cleanly unless the workflow-only nature of the change makes it inapplicable; if inapplicable, record the exact reason in task notes.
- [ ] `make lint` — passes cleanly unless the workflow-only nature of the change makes it inapplicable; if inapplicable, record the exact reason in task notes.
</acceptance_criteria>
