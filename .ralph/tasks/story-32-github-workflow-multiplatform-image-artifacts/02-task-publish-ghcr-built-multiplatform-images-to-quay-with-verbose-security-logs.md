## Task: Publish GHCR Built Multiplatform Images To Quay With Verbose Security Logs <status>not_started</status> <passes>false</passes>

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
- [ ] The workflow publishes the final multi-platform `runner-image` and `verify-image` images to Quay after GHCR publishing succeeds.
- [ ] Quay publishing does not run Nix and does not rebuild images; it copies or pushes already-built multi-platform images/manifests.
- [ ] Quay destination references use `vars.QUAY_ORGANIZATION`, `vars.RUNNER_IMAGE_REPOSITORY`, and `vars.VERIFY_IMAGE_REPOSITORY`.
- [ ] The workflow does not hardcode `cockroach_migrate_tool`, `runner`, or `verify`; it gets those values from the already-configured GitHub vars at runtime.
- [ ] The workflow does not add manual setup steps, workflow inputs, checked-in defaults, or fallback values for Quay organization/repository names.
- [ ] Quay authentication uses `secrets.QUAY_ROBOT_USERNAME` and `secrets.QUAY_ROBOT_PASSWORD`.
- [ ] The workflow does not add manual setup steps, workflow inputs, checked-in defaults, or fallback values for Quay robot credentials.
- [ ] Workflow logs do not print Quay passwords, tokens, or secret values.
- [ ] The only Quay tags published by this task are exact git commit SHA tags for the runner and verify images.
- [ ] Quay publish logs show source GHCR refs/digests, destination Quay refs/digests, manifest list details, and included platforms.
- [ ] The workflow queries or otherwise inspects Quay security/vulnerability status for both pushed images after publish.
- [ ] GitHub Actions logs show Quay vulnerability/security results verbosely, including available severity counts or an explicit scanner pending/unavailable/error state.
- [ ] The implementation defines and records the vulnerability policy: whether vulnerabilities fail the workflow or are reported only.
- [ ] Scanner/API/authentication errors are not swallowed. They either fail the workflow or are explicitly handled according to the documented policy with clear log output.
- [ ] Quay publish and vulnerability inspection happen only after the five parallel jobs and GHCR publish have succeeded.
- [ ] The workflow has no `concurrency` attribute and no cancellation/superseding behavior; every push creates a full run even when a previous run is still active.
- [ ] Manual workflow syntax verification passes locally or through a workflow linter, and the exact command is recorded in task notes.
- [ ] Manual hosted verification: trigger the workflow on GitHub, inspect authenticated workflow logs with `/home/joshazimullah.linux/github-api-curl` or an equivalent authenticated API path, and record evidence that Quay publish happened after GHCR publish.
- [ ] Manual hosted registry verification: inspect Quay and record evidence that both commit-SHA-only multi-platform tags exist at the organization/repositories resolved from the already-configured GitHub vars.
- [ ] Manual hosted security-log verification: record the workflow log section showing Quay vulnerability/security output for both images, including any pending/unavailable/error state.
- [ ] `make check` — passes cleanly unless the workflow-only nature of the change makes it inapplicable; if inapplicable, record the exact reason in task notes.
- [ ] `make lint` — passes cleanly unless the workflow-only nature of the change makes it inapplicable; if inapplicable, record the exact reason in task notes.
</acceptance_criteria>

<plan>.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/02-task-publish-ghcr-built-multiplatform-images-to-quay-with-verbose-security-logs_plans/2026-04-28-quay-publish-security-plan.md</plan>
