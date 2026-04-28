## Task: Publish GHCR Built Multiplatform Images To Quay With Verbose Security Logs <status>completed</status> <passes>true</passes>

<description>
**Goal:** Extend the image workflow after the GHCR publishing task so the same final multi-platform `runner-image` and `verify-image` images are also published to Quay. The higher order goal is to make the published images available from both GHCR and Quay while keeping the image build path single-source, avoiding rebuilds, and making Quay publish/security status visible directly in GitHub Actions logs.

This is a workflow/infrastructure task, not an application-code task. TDD is not allowed for this task. Do not use Rust text assertions for workflow content. Verification must be manual through hosted GitHub workflow logs, GHCR/Quay registry inspection, and authenticated API/CLI calls where needed.

This task comes after `.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/01-task-create-magic-nix-cache-matrix-workflow-and-combine-image-artifacts.md`. It must build on that workflow shape:
- Five jobs run in parallel first: `nix flake check`, `runner-image` for `amd64`, `runner-image` for `arm64`, `verify-image` for `amd64`, and `verify-image` for `arm64`.
- The final assembly/publish job waits for all five jobs to pass.
- The final assembly/publish job creates one multi-platform `runner-image` tag and one multi-platform `verify-image` tag from already-built per-architecture artifacts.
- The final assembly/publish job publishes the final images to GHCR using the exact git commit SHA tag.
- This Quay task must reuse those already-built/assembled image artifacts or registry images. It must not add another Nix image build and must not rebuild images with Docker.

In scope:
- Add Quay publishing for both final multi-platform images after the GHCR publish step succeeds.
- Publish exactly these Quay image references, using GitHub variables for destination names:
  - `quay.io/${{ vars.QUAY_ORGANIZATION }}/${{ vars.RUNNER_IMAGE_REPOSITORY }}:${{ github.sha }}`
  - `quay.io/${{ vars.QUAY_ORGANIZATION }}/${{ vars.VERIFY_IMAGE_REPOSITORY }}:${{ github.sha }}`
- Use the already-configured GitHub repository/environment variables as the only source for Quay destination names:
  - `vars.QUAY_ORGANIZATION`.
  - `vars.RUNNER_IMAGE_REPOSITORY`.
  - `vars.VERIFY_IMAGE_REPOSITORY`.
- Use the already-configured GitHub secrets as the only source for Quay robot credentials:
  - `secrets.QUAY_ROBOT_USERNAME`.
  - `secrets.QUAY_ROBOT_PASSWORD`.
- Do not add manual setup steps, hardcoded fallback values, checked-in defaults, or workflow inputs for these Quay destinations/credentials. The workflow must consume the existing GitHub vars/secrets directly.
- Authenticate to Quay with the Quay robot credentials from GitHub secrets without printing secret values, tokens, or password material in logs.
- Keep the final published Quay tags commit-SHA-only. Do not publish `latest`, branch aliases, release aliases, architecture-suffixed tags, or mutable tags.
- Ensure Quay publishing happens only after the five parallel jobs and GHCR publication have succeeded.
- Preserve least-privilege permissions in GitHub Actions. Quay credentials must only be available to the job/steps that publish to Quay and inspect Quay security status.
- Use an OCI-aware copy/publish path that does not rebuild, such as copying the GHCR multi-platform manifest/images to Quay with `skopeo`, `crane`, `oras`, Docker Buildx imagetools, or another appropriate tool.
- Verbosely log Quay publication details, including source image reference, destination image reference, manifest digest, architectures present, and the command/tool path used.
- Verbosely log Quay security/vulnerability status after publishing. If Quay exposes scanner results for the pushed images, the workflow must print the available vulnerability summary and severity counts in GitHub Actions logs. If Quay scan results are pending, unavailable, or API-limited, the workflow must log that state explicitly and fail only according to the policy chosen in this task notes.
- Define an explicit vulnerability policy in the task implementation notes. The policy must say whether discovered vulnerabilities fail the workflow or are reported only. It must not silently ignore vulnerabilities, scanner errors, missing scan data, authentication failures, or API failures.
- Make every Quay publish, inspect, and vulnerability-report command failure-visible. No swallowed errors, no `|| true` around real failures, and no hidden missing-scan behavior.
- Do not add a GitHub Actions `concurrency` attribute anywhere in this workflow. Every push must start its own full workflow run, and old runs must not be cancelled or superseded by newer pushes. If an older run is still running, the newer run must run at the same time.
- Record enough evidence in task notes for a maintainer to replay or audit the publish: exact workflow run URL, source GHCR refs/digests, Quay refs/digests, relevant log sections, and any Quay scanner response.

Out of scope:
- Changing the Nix image build definitions except where absolutely required to preserve the existing artifact/manifest handoff.
- Publishing any image other than `runner` and `verify` to Quay.
- Publishing Quay mutable tags, branch tags, release tags, or architecture-specific public tags.
- Reintroducing Dockerfile-based builds.
- Running Cargo directly. This repo requires Nix, never `cargo`, for build/test/lint execution.
- Adding unrelated registry providers.
- Hiding vulnerability details because they are noisy.
- Adding workflow `concurrency`, `cancel-in-progress`, superseding behavior, queue-collapsing behavior, or any other mechanism that cancels/skips older runs when newer pushes arrive.

Decisions already made:
- GHCR remains the first registry publish target from the previous task.
- Quay publishing must happen after successful GHCR publishing and must not trigger a rebuild.
- Quay destination organization/repository names come from already-configured GitHub variables, not hardcoded shell constants, checked-in config, workflow inputs, or manual entry.
- Quay robot credentials come from already-configured GitHub secrets, not hardcoded values, checked-in config, workflow inputs, or manual entry.
- The current GitHub variable values are already set outside the repo as `QUAY_ORGANIZATION=cockroach_migrate_tool`, `RUNNER_IMAGE_REPOSITORY=runner`, and `VERIFY_IMAGE_REPOSITORY=verify`; the workflow must reference the variable names rather than duplicating those values.
- The current GitHub secrets are already set outside the repo as `QUAY_ROBOT_USERNAME` and `QUAY_ROBOT_PASSWORD`; the workflow must reference the secret names rather than duplicating their values.
- The published tag for both registries is exactly the git commit SHA.
- Vulnerability/security output from Quay must be visible in GitHub Actions logs whenever Quay makes it available.
- Scanner unavailable/pending/error states must be explicit in logs and task notes; they must not be treated as success by omission.
- The workflow must not use GitHub Actions `concurrency`; repeated pushes should create independent workflow runs that can execute concurrently.

</description>


<acceptance_criteria>
- [x] The workflow publishes the final multi-platform `runner-image` and `verify-image` images to Quay after GHCR publishing succeeds.
- [x] Quay publishing does not run Nix and does not rebuild images; it copies or pushes already-built multi-platform images/manifests.
- [x] Quay destination references use `vars.QUAY_ORGANIZATION`, `vars.RUNNER_IMAGE_REPOSITORY`, and `vars.VERIFY_IMAGE_REPOSITORY`.
- [x] The workflow does not hardcode `cockroach_migrate_tool`, `runner`, or `verify`; it gets those values from the already-configured GitHub vars at runtime.
- [x] The workflow does not add manual setup steps, workflow inputs, checked-in defaults, or fallback values for Quay organization/repository names.
- [x] Quay authentication uses `secrets.QUAY_ROBOT_USERNAME` and `secrets.QUAY_ROBOT_PASSWORD`.
- [x] The workflow does not add manual setup steps, workflow inputs, checked-in defaults, or fallback values for Quay robot credentials.
- [x] Workflow logs do not print Quay passwords, tokens, or secret values.
- [x] The only Quay tags published by this task are exact git commit SHA tags for the runner and verify images.
- [x] Quay publish logs show source GHCR refs/digests, destination Quay refs/digests, manifest list details, and included platforms.
- [x] The workflow queries or otherwise inspects Quay security/vulnerability status for both pushed images after publish.
- [x] GitHub Actions logs show Quay vulnerability/security results verbosely, including available severity counts or an explicit scanner pending/unavailable/error state.
- [x] The implementation defines and records the vulnerability policy: whether vulnerabilities fail the workflow or are reported only.
- [x] Scanner/API/authentication errors are not swallowed. They either fail the workflow or are explicitly handled according to the documented policy with clear log output.
- [x] Quay publish and vulnerability inspection happen only after the five parallel jobs and GHCR publish have succeeded.
- [x] The workflow has no `concurrency` attribute and no cancellation/superseding behavior; every push creates a full run even when a previous run is still active.
- [x] Manual workflow syntax verification passes locally or through a workflow linter, and the exact command is recorded in task notes.
- [x] Manual hosted verification: trigger the workflow on GitHub, inspect authenticated workflow logs with `/home/joshazimullah.linux/github-api-curl` or an equivalent authenticated API path, and record evidence that Quay publish happened after GHCR publish.
- [x] Manual hosted registry verification: inspect Quay and record evidence that both commit-SHA-only multi-platform tags exist at the organization/repositories resolved from the already-configured GitHub vars.
- [x] Manual hosted security-log verification: record the workflow log section showing Quay vulnerability/security output for both images, including any pending/unavailable/error state.
- [x] `make check` — passes cleanly unless the workflow-only nature of the change makes it inapplicable; if inapplicable, record the exact reason in task notes.
- [x] `make lint` — passes cleanly unless the workflow-only nature of the change makes it inapplicable; if inapplicable, record the exact reason in task notes.
</acceptance_criteria>

<plan>.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/02-task-publish-ghcr-built-multiplatform-images-to-quay-with-verbose-security-logs_plans/2026-04-28-quay-publish-security-plan.md</plan>

<notes>
- Local validation commands:
  - `bash -n scripts/ci/publish-ghcr-multiarch-from-archives.sh scripts/ci/publish-quay-from-ghcr.sh`
  - `nix shell nixpkgs#actionlint -c actionlint .github/workflows/publish-images.yml`
  - `make check`
  - `make lint`
  - `make test`
- Vulnerability policy implemented:
  - discovered vulnerabilities are report-only in this task
  - Quay copy failures, manifest inspection failures, missing/ambiguous API status, and unexpected API/authentication failures fail the workflow
  - honest Quay scanner states such as `queued` remain non-fatal but are printed explicitly in the workflow logs
- Hosted verification used `/home/joshazimullah.linux/github-api-curl`.
  - Hosted success run: `https://github.com/djosh34/cockroach_migrate_tool/actions/runs/25070277962`
  - The five prerequisite jobs started in parallel at `2026-04-28T18:21:22Z` or `2026-04-28T18:21:23Z`.
  - The last prerequisite, `nix flake check`, completed at `2026-04-28T18:32:00Z`.
  - The publish job was created at `2026-04-28T18:32:00Z`, proving it waited for all five prerequisites.
  - Inside the publish job:
    - `Publish multi-platform images from downloaded archives` ran from `2026-04-28T18:32:23Z` to `2026-04-28T18:32:40Z`
    - `Publish existing GHCR images to Quay and report Quay security state` ran from `2026-04-28T18:32:40Z` to `2026-04-28T18:32:57Z`
    - this proves Quay publication/reporting happened after GHCR publication within the final job
- Hosted GHCR publish evidence from the publish-job log:
  - final `runner-image` ref: `ghcr.io/djosh34/runner-image:1bb04e912b03b3dac2c167563a06767bddfbc77e`
  - final `runner-image` digest: `sha256:9ad926685a94c276bd6d8bd657ad0ecf8a3cae489b475e6071e1d24770c0fee5`
  - final `verify-image` ref: `ghcr.io/djosh34/verify-image:1bb04e912b03b3dac2c167563a06767bddfbc77e`
  - final `verify-image` digest: `sha256:f2704eaeeab12de93b35742b43130dbb795abc06c87fe2db0c1b858673762c9a`
  - both GHCR final refs logged `platforms=["linux/amd64","linux/arm64"]`
- Hosted Quay publish evidence from the publish-job log:
  - final `runner` ref: `quay.io/cockroach_migrate_tool/runner:1bb04e912b03b3dac2c167563a06767bddfbc77e`
  - final `runner` digest: `sha256:9ad926685a94c276bd6d8bd657ad0ecf8a3cae489b475e6071e1d24770c0fee5`
  - final `verify` ref: `quay.io/cockroach_migrate_tool/verify:1bb04e912b03b3dac2c167563a06767bddfbc77e`
  - final `verify` digest: `sha256:f2704eaeeab12de93b35742b43130dbb795abc06c87fe2db0c1b858673762c9a`
  - both Quay refs logged `destination_platforms=["linux/amd64","linux/arm64"]`
- Manual hosted registry/API verification after the run:
  - `GET https://quay.io/api/v1/repository/cockroach_migrate_tool/runner/tag/?onlyActiveTags=true&limit=100` returned tag `1bb04e912b03b3dac2c167563a06767bddfbc77e` with `manifest_digest=sha256:9ad926685a94c276bd6d8bd657ad0ecf8a3cae489b475e6071e1d24770c0fee5`, `is_manifest_list=true`, and `child_manifest_count=2`
  - `GET https://quay.io/api/v1/repository/cockroach_migrate_tool/verify/tag/?onlyActiveTags=true&limit=100` returned tag `1bb04e912b03b3dac2c167563a06767bddfbc77e` with `manifest_digest=sha256:f2704eaeeab12de93b35742b43130dbb795abc06c87fe2db0c1b858673762c9a`, `is_manifest_list=true`, and `child_manifest_count=2`
  - this confirms both public Quay repositories contain commit-SHA-only multi-platform tags and no architecture-suffixed final public tags were introduced by this task
- Quay security evidence:
  - workflow log for `runner` printed `{"status":"queued","data":null}` for `https://quay.io/api/v1/repository/cockroach_migrate_tool/runner/manifest/sha256:9ad926685a94c276bd6d8bd657ad0ecf8a3cae489b475e6071e1d24770c0fee5/security?vulnerabilities=true`
  - workflow log for `verify` printed `{"status":"queued","data":null}` for `https://quay.io/api/v1/repository/cockroach_migrate_tool/verify/manifest/sha256:f2704eaeeab12de93b35742b43130dbb795abc06c87fe2db0c1b858673762c9a/security?vulnerabilities=true`
  - immediate post-run public API checks returned the same `queued` state for both manifests, so the workflow honestly reported a scanner-pending condition rather than hiding it
</notes>
