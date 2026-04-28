# Plan: Publish The Existing GHCR Multi-Platform Images To Quay And Report Quay Security State

## References

- Task:
  - `.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/02-task-publish-ghcr-built-multiplatform-images-to-quay-with-verbose-security-logs.md`
- Current workflow:
  - `.github/workflows/publish-images.yml`
- Existing GHCR publish boundary:
  - `scripts/ci/publish-ghcr-multiarch-from-archives.sh`
- Repo quality lanes:
  - `Makefile`
- Hosted log/API auth path:
  - `/home/joshazimullah.linux/github-api-curl`
- Skills:
  - `improve-code-boundaries`
  - `tdd`

## Planning Assumptions

- This turn is planning-only because the task had no `<plan>` pointer and no execution marker.
- The task markdown is treated as approval for the public workflow interface and behavior priorities in this planning turn.
- This task is a workflow/infrastructure TDD exception.
  - execution must not add fake Rust string-assertion tests for YAML content
  - validation must happen through honest workflow syntax checks, script execution, hosted workflow runs, and registry/API inspection
- The existing workflow shape from story 32 task 01 is already correct:
  - four per-architecture image builds plus `nix flake check`
  - one final publish job after all five prerequisites pass
- The publish job must stay the only place that can see Quay credentials.
- If execution proves the Quay copy/report path cannot be expressed cleanly with the planned script boundaries, or that Quay scan state must be handled with a materially different policy than planned here, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Current State Summary

- `.github/workflows/publish-images.yml` already has the required five parallel prerequisite jobs and one final `publish-multiarch` job.
- The final job currently:
  - downloads the four image artifacts
  - logs into GHCR
  - enables Docker Buildx
  - runs `scripts/ci/publish-ghcr-multiarch-from-archives.sh`
- `publish-ghcr-multiarch-from-archives.sh` already owns the correct artifact-to-GHCR boundary:
  - validate required env
  - load the four archives
  - push temporary per-architecture refs
  - assemble one final `runner-image:${GIT_SHA}` manifest
  - assemble one final `verify-image:${GIT_SHA}` manifest
  - inspect the final GHCR refs
- Nothing currently handles:
  - Quay authentication
  - Quay destination ref construction from GitHub vars
  - copying the already-published GHCR multi-platform refs to Quay
  - Quay manifest/digest/platform reporting
  - Quay vulnerability/security reporting

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - Quay publish logic could easily end up split across raw YAML shell snippets, the GHCR archive-publish script, and ad hoc `curl` calls
- Required boundary shape during execution:
  - keep `.github/workflows/publish-images.yml` thin and orchestration-only
  - keep `publish-ghcr-multiarch-from-archives.sh` responsible only for `artifact archives -> GHCR refs`
  - add one Quay-focused script responsible only for `GHCR final refs -> Quay refs + Quay reporting`
- Concrete cleanup goal:
  - the workflow should wire tools, permissions, and secrets
  - the GHCR script should emit structured publish outputs for downstream consumption
  - the Quay script should own ref construction, `skopeo` copy, manifest inspection, and vulnerability-report API handling
- Smells to avoid:
  - expanding the workflow with long inline shell blocks for Quay copy/report
  - renaming the GHCR script into a misleading generic blob that mixes local archive loading with remote registry API polling
  - introducing a generic shared helper library for two scripts when direct shell functions are sufficient

## Intended Public Contract After Execution

- The existing publish workflow still builds images exactly once via Nix in the matrix jobs and never rebuilds them in the final publish job.
- After GHCR publish succeeds, the same final multi-platform images are copied to:
  - `quay.io/${{ vars.QUAY_ORGANIZATION }}/${{ vars.RUNNER_IMAGE_REPOSITORY }}:${{ github.sha }}`
  - `quay.io/${{ vars.QUAY_ORGANIZATION }}/${{ vars.VERIFY_IMAGE_REPOSITORY }}:${{ github.sha }}`
- Quay destination names come only from:
  - `vars.QUAY_ORGANIZATION`
  - `vars.RUNNER_IMAGE_REPOSITORY`
  - `vars.VERIFY_IMAGE_REPOSITORY`
- Quay auth comes only from:
  - `secrets.QUAY_ROBOT_USERNAME`
  - `secrets.QUAY_ROBOT_PASSWORD`
- Workflow logs must show, for both images:
  - source GHCR ref
  - source GHCR digest
  - destination Quay ref
  - destination Quay digest
  - included architectures/platforms
  - copy tool/command path used
  - explicit Quay scanner state or vulnerability summary

## Vulnerability Policy To Implement

- Policy for discovered vulnerabilities:
  - report-only in this task
  - if Quay returns real scan results, print the available severity counts and status to GitHub Actions logs
  - discovered vulnerabilities alone do not fail this workflow in this task
- Policy for scanner/system failures:
  - fail the workflow if Quay authentication fails
  - fail the workflow if the Quay copy command fails
  - fail the workflow if manifest inspection fails
  - fail the workflow if the Quay security API call fails unexpectedly
  - fail the workflow if the script cannot determine whether scan data is available
- Policy for honest non-ready states:
  - if Quay truthfully reports that scanning is pending, unavailable, or not yet produced for the pushed image, log that state explicitly and do not fail only for that reason
  - execution must not hide that state or treat silence as success

## Tooling And Workflow Strategy

- Prefer `skopeo copy --all` for the Quay publish path because it copies the already-published multi-platform image/manifests from GHCR to Quay without rebuilding.
- Use `skopeo inspect` for source/destination manifest and platform reporting where possible.
- Use `curl` plus `jq` for Quay security/vulnerability API reporting.
- Install required publish-job tools directly on the GitHub runner.
  - expected tools: `skopeo`, `jq`
  - do not use Nix in the final publish job
- Keep Docker/Buildx only for the existing GHCR assembly step.

## Files And Structure To Change

- [ ] `.github/workflows/publish-images.yml`
  - keep the existing five-job topology
  - keep GHCR publish first
  - add Quay-only tool installation in the final job
  - add Quay login/copy/report steps after GHCR publish
  - scope Quay secrets only to the Quay publish/report step or steps
- [ ] `scripts/ci/publish-ghcr-multiarch-from-archives.sh`
  - keep ownership of archive loading and GHCR manifest creation
  - emit a structured summary file for the final GHCR refs/digests/platforms so downstream steps do not re-derive them from logs
- [ ] `scripts/ci/publish-quay-from-ghcr.sh`
  - new script
  - consume the GHCR publish summary and GitHub vars/secrets from env
  - copy final GHCR refs to Quay without rebuild
  - inspect Quay manifests/platforms after copy
  - query Quay security status and print an explicit summary
- [ ] `.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/02-task-publish-ghcr-built-multiplatform-images-to-quay-with-verbose-security-logs.md`
  - update acceptance checkboxes and notes after execution
  - record hosted workflow URL, refs, digests, and Quay scanner evidence

## Execution Slices For The Next Turn

### Slice 1: Preserve The Existing Publish Boundary While Emitting Honest GHCR Outputs

- [ ] Modify `publish-ghcr-multiarch-from-archives.sh` so it can write a machine-readable publish summary for:
  - final `runner-image` GHCR ref and digest
  - final `verify-image` GHCR ref and digest
  - included platforms where practical
- [ ] Keep the workflow thin by consuming that summary in later steps rather than scraping human log text
- [ ] Run local workflow syntax validation immediately after this slice
- Stop condition:
  - if the current GHCR script cannot emit stable publish outputs without becoming muddier than the workflow itself, switch back to `TO BE VERIFIED`

### Slice 2: Add Quay Copy Without Rebuild

- [ ] Add `scripts/ci/publish-quay-from-ghcr.sh`
- [ ] Update the publish job to install `skopeo` and `jq`
- [ ] Pass only the required Quay vars/secrets/env into the Quay script
- [ ] Use `skopeo copy --all` or an equivalent OCI-aware copy path to publish:
  - GHCR `runner-image:${GIT_SHA}` -> Quay `runner:${GIT_SHA}`
  - GHCR `verify-image:${GIT_SHA}` -> Quay `verify:${GIT_SHA}`
- [ ] Log source and destination refs plus inspected digests/platforms
- [ ] Re-run local workflow syntax validation after this slice
- Stop condition:
  - if the copy path requires a rebuild, architecture-specific public tags, or YAML-level duplication of ref logic, switch back to `TO BE VERIFIED`

### Slice 3: Add Explicit Quay Security Reporting With The Chosen Policy

- [ ] Query Quay security state for both final Quay refs after publish
- [ ] Print either:
  - real vulnerability summary/severity counts
  - or an explicit pending/unavailable/not-scanned-yet state
- [ ] Fail on auth/API/command ambiguity or hidden missing-data conditions
- [ ] Keep discovered vulnerabilities report-only in this task, and record that policy in task notes
- [ ] Re-run local workflow syntax validation after this slice
- Stop condition:
  - if Quay’s real API shape requires a materially different non-ambiguous policy than planned here, switch back to `TO BE VERIFIED`

### Slice 4: Hosted Verification And Evidence Collection

- [ ] Push the branch and trigger the hosted workflow normally
- [ ] Inspect the workflow run with `/home/joshazimullah.linux/github-api-curl`
- [ ] Record:
  - exact workflow run URL
  - proof that the publish job started only after the five prerequisites passed
  - GHCR refs/digests
  - Quay refs/digests
  - Quay platform/manifests evidence
  - Quay security-log evidence for both images
- [ ] Inspect Quay directly to confirm both commit-SHA-only multi-platform tags exist at the variable-resolved org/repositories

### Slice 5: Required Repository Validation And Final Boundary Review

- [ ] Run `make check`
- [ ] Run `make lint`
- [ ] Run `make test`
- [ ] Do not run `make test-long`
- [ ] Do one final `improve-code-boundaries` review:
  - workflow YAML remains thin
  - GHCR archive publish logic still has one owner
  - Quay copy/report logic still has one owner
  - no swallowed errors or `|| true`

## Planned Local Validation Commands

- `nix shell nixpkgs#actionlint -c actionlint .github/workflows/publish-images.yml`
- `make check`
- `make lint`
- `make test`

## Expected Outcome

- The workflow will keep one honest build path and one honest publish order:
  - Nix builds archives once
  - GHCR assembles the multi-platform images once
  - Quay copies those exact final images without rebuild
- The publish job will expose enough registry/security detail in logs for maintainers to audit a run without guessing.
- The code boundary will stay cleaner than a YAML-only implementation because archive publishing and Quay reporting will have separate script owners.

Plan path: `.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/02-task-publish-ghcr-built-multiplatform-images-to-quay-with-verbose-security-logs_plans/2026-04-28-quay-publish-security-plan.md`

NOW EXECUTE
