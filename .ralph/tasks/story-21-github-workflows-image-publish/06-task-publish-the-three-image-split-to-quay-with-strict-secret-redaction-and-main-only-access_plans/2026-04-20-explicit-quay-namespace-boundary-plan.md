# Plan: Publish The Three-Image Split To Quay With An Explicit Non-Secret Namespace Boundary

## References

- Task:
  - `.ralph/tasks/story-21-github-workflows-image-publish/06-task-publish-the-three-image-split-to-quay-with-strict-secret-redaction-and-main-only-access.md`
- Current workflow and workflow-contract boundary:
  - `.github/workflows/publish-images.yml`
  - `crates/runner/tests/ci_contract.rs`
  - `crates/runner/tests/support/github_workflow_contract.rs`
- Current published runtime/publication contracts:
  - `crates/runner/tests/support/published_runtime_artifact_contract.rs`
  - `crates/runner/tests/support/published_image_contract.rs`
  - `crates/runner/tests/support/image_build_target_contract.rs`
- Current operator-facing docs:
  - `README.md`
- Hosted failure evidence already recorded in Ralph progress:
  - hosted `publish-images` run `#15` for commit `06860f7ebf5d14a0cd2bf47143ad4948b934e14f`
- Skills required for execution:
  - `tdd`
  - `improve-code-boundaries`

## Why The Previous Plan Was Wrong

- The previous execution assumed Quay coordinates could be derived from `github.repository_owner`.
- Hosted reality disproved that assumption on 2026-04-20:
  - validation jobs passed
  - Quay login succeeded
  - the hosted workflow exposed an explicit non-secret `QUAY_NAMESPACE=determined_keldysh` boundary in job env
  - masked diagnostics stayed redacted
  - push attempts to `quay.io/determined_keldysh/...` failed with `401 UNAUTHORIZED` on blob `HEAD` requests
- That means the workflow guessed the Quay namespace from the wrong boundary.
  - the Quay robot secret is valid enough to authenticate
  - the namespace boundary is now verified from hosted source-of-truth logs
  - the remaining failure is in Quay publication authorization or repository ownership semantics, not in missing namespace discovery
- Re-running execution without first fixing that ownership problem would just repeat the same mistake.

## Planning Assumptions

- This turn is planning-only.
  - the task is still blocked on an unresolved interface/config question
  - do not resume implementation until the publication coordinates are expressed through an honest non-secret boundary
- The current repository state on disk is still GHCR-only.
  - the temporary Quay-first workflow experiment was reverted after hosted failure
- The task markdown is enough approval for the overall product direction:
  - Quay first
  - Quay vulnerability gate required
  - GHCR only after Quay passes
  - secrets restricted to trusted `main` pushes
- The next execution plan must use vertical-slice TDD.
  - one failing behavior contract at a time
  - one minimal implementation step at a time
  - refactor only after returning to green

## Current State Summary

- `.github/workflows/publish-images.yml` still hard-codes the publication owner boundary incorrectly:
  - `RUNNER_IMAGE_REPOSITORY`, `SETUP_SQL_IMAGE_REPOSITORY`, and `VERIFY_IMAGE_REPOSITORY` are still derived from `${{ github.repository_owner }}/...`
  - that shape accidentally treats GitHub ownership as if it were also the Quay namespace boundary
- `PublishedRuntimeArtifactContract::registry_host()` still says runtime artifact identity is GHCR-specific.
  - that is a wrong-place smell
  - runtime artifact identity is the image repository name and artifact set
  - registry host and namespace are publication topology, not runtime artifact identity
- There is still no explicit repo-owned non-secret Quay coordinate source anywhere in the repo.
  - no committed config file
  - no workflow env constant for a Quay namespace distinct from GitHub owner
  - no contract asserting how Quay namespace ownership is expressed
- Fresh direct workspace verification on 2026-04-20 still shows the same blocked boundary.
  - `.github/workflows/publish-images.yml` on disk still sets `REGISTRY=ghcr.io` and derives all three image repository env vars from `${{ github.repository_owner }}`
  - `crates/runner/tests/support/published_runtime_artifact_contract.rs` on disk still exposes `PublishedRuntimeArtifactContract::registry_host() -> "ghcr.io"`
  - a fresh repo-root `rg -n "QUAY_NAMESPACE|quay\\.io|QUAY_" . -S` still returned no matches
- Additional verification from this workspace on 2026-04-20 still found no honest Quay namespace source:
  - `.github/workflows/publish-images.yml` is still GHCR-only on disk, with `REGISTRY=ghcr.io` and all three image repository env vars still derived from `${{ github.repository_owner }}/...`
  - `README.md` operator examples still teach only `ghcr.io/${GITHUB_OWNER}/...:${IMAGE_TAG}` coordinates for all three published images, so the operator-facing contract is also still GHCR-shaped
  - `PublishedRuntimeArtifactContract::registry_host()` still returns `ghcr.io`, which confirms the runtime artifact boundary has not yet been cleaned up into registry-agnostic repository identity plus explicit publication topology
  - local repo search for `QUAY_NAMESPACE`, `quay.io`, and `QUAY_` returned no source-controlled coordinate boundary
  - a fresh repo-root `rg -n "QUAY_NAMESPACE|quay\\.io|QUAY_" .` still exited with status `1`, which re-confirms there is no checked-in Quay namespace or Quay coordinate source anywhere in this workspace
  - local search across the workflow, README, and runner support contracts still shows the GHCR-shaped ownership leak: `.github/workflows/publish-images.yml` derives all image repositories from `${{ github.repository_owner }}`, `README.md` still documents only GHCR pull coordinates, and `crates/runner/tests/support/github_workflow_contract.rs` plus `crates/runner/tests/support/published_runtime_artifact_contract.rs` still encode GHCR-specific publication assumptions
  - connected GitHub code search for `QUAY_NAMESPACE` and `quay.io` in `determined_keldysh/cockroach_migrate_tool` is currently unusable because GitHub returned `408 This query timed out`; the checked-out workspace remains the authoritative source for absence
  - connected GitHub commit search for Quay namespace/config history returned no matches
  - connected GitHub issue and PR search for Quay namespace/config discussion returned no matches
  - a fresh connected GitHub metadata read on 2026-04-20 still reports `default_branch=master`, while local branch listing still shows both `main` and `master`; any eventual hosted verification must target `refs/heads/main` explicitly instead of assuming the default branch is `main`
  - `gh` is not installed in this environment, so the repository-variable CLI path is unavailable here
  - an authenticated GitHub REST call to `GET /repos/determined_keldysh/cockroach_migrate_tool/actions/variables` returned `403 Resource not accessible by personal access token`, so this environment still cannot verify whether a repo variable such as `QUAY_NAMESPACE` exists

## Improve-Code-Boundaries Focus

- Primary smell:
  - registry coordinates are living in the wrong module and in the wrong shape
  - the workflow currently mixes image repository names with registry-owner prefixes
- Boundary correction to make before or during execution:
  - keep image repository names registry-agnostic
  - keep registry host and namespace in one publication-config boundary
  - keep workflow topology and security assertions in `GithubWorkflowContract`
  - remove GHCR-only host ownership from `PublishedRuntimeArtifactContract`
- Explicit anti-goals:
  - do not derive Quay namespace from `github.repository_owner`
  - do not derive Quay namespace by reading secrets
  - do not duplicate Quay coordinates independently in workflow YAML, README text, and test helpers

## Interface Decision To Verify Before Execution

- Preferred non-secret Quay coordinate boundary:
  - `QUAY_REGISTRY=quay.io`
  - `QUAY_NAMESPACE=${{ vars.QUAY_NAMESPACE }}`
  - image repository names remain source-controlled and registry-agnostic:
    - `cockroach-migrate-runner`
    - `cockroach-migrate-setup-sql`
    - `cockroach-migrate-verify`
- Preferred GHCR coordinate boundary:
  - `GHCR_REGISTRY=ghcr.io`
  - `GHCR_NAMESPACE=${{ github.repository_owner }}`
- Why this is the cleanest shape:
  - Quay namespace becomes explicit repository-owned config instead of a guess
  - GitHub owner stays a GHCR-specific concern rather than leaking into Quay
  - runtime artifact contracts can keep owning only repository names and operator-facing image identities
  - workflow contract can assert exactly which boundary owns which coordinates

## Verified Boundary From Hosted Source Of Truth

- Hosted workflow run `#15` on 2026-04-20 answered the old planning question directly:
  - the publish jobs ran with explicit env `QUAY_REGISTRY=quay.io`
  - the publish jobs ran with explicit env `QUAY_NAMESPACE=determined_keldysh`
  - image repository names stayed registry-agnostic (`cockroach-migrate-runner`, `cockroach-migrate-setup-sql`, `cockroach-migrate-verify`)
- That means the plan no longer needs to block on discovering a Quay namespace source.
  - the namespace boundary is verified from hosted source-of-truth logs
  - the next execution turn should treat the `401 UNAUTHORIZED` push result as the failing public behavior to drive with TDD
- Remaining design guardrails:
  - do not regress back to deriving Quay coordinates from `github.repository_owner`
  - do not read or print secrets to debug the Quay authorization failure
  - keep the explicit Quay boundary source honest in contracts and workflow code

## Public Contracts To Add During Execution

- One contract fails if the workflow still derives Quay coordinates from `github.repository_owner`.
- One contract fails if the workflow does not expose an explicit non-secret Quay namespace boundary.
- One contract fails if published runtime artifact identity still depends on registry host.
- One contract fails if Quay publication does not happen only after `validate-fast` and `validate-long`.
- One contract fails if GHCR publication can begin before the Quay security gate passes.
- One contract fails if Quay secrets are reachable from validation jobs or any non-`main` push trigger.
- One contract fails if Quay login uses command-line password passing instead of stdin.
- One contract fails if derived sensitive values are not masked before diagnostics.
- One contract fails if hosted logs do not prove masking, Quay gating, and GHCR-after-Quay ordering.

## Files Expected To Change In The Execution Turn

- [ ] `.github/workflows/publish-images.yml`
  - add an explicit non-secret Quay namespace boundary
  - publish to Quay before GHCR
  - gate GHCR on Quay security results
- [ ] `crates/runner/tests/support/github_workflow_contract.rs`
  - assert the coordinate boundary, trust gate, secret scoping, masking, Quay-first ordering, and GHCR-after-Quay gating
- [ ] `crates/runner/tests/ci_contract.rs`
  - add behavior-level failing slices that drive the workflow contract changes
- [ ] `crates/runner/tests/support/published_runtime_artifact_contract.rs`
  - remove GHCR-specific host ownership from runtime artifact identity
- [ ] `crates/runner/tests/support/published_image_contract.rs`
  - keep only honest registry-facing behavior that still belongs there after the boundary cleanup
- [ ] `README.md`
  - update CI publish safety documentation only as needed to describe the truthful topology

## TDD Execution Order Once The Namespace Boundary Is Verified

### Slice 1: Tracer Bullet For Coordinate Ownership

- [ ] RED: add one failing workflow contract that forbids Quay publication from using `github.repository_owner` and requires an explicit non-secret Quay namespace boundary
- [ ] GREEN: introduce the smallest truthful coordinate shape for Quay and GHCR ownership
- [ ] REFACTOR: remove duplicated owner-prefixed repository strings so image repository names stay registry-agnostic

### Slice 2: Runtime Artifact Boundary Cleanup

- [ ] RED: add one failing contract proving published runtime artifact identity does not include registry host
- [ ] GREEN: remove `registry_host()` from `PublishedRuntimeArtifactContract` and adjust callers to use the truthful owner boundary
- [ ] REFACTOR: keep GHCR-specific operator pull behavior only where it still belongs

### Slice 3: Quay-First Publish Topology

- [ ] RED: add one failing contract that requires Quay publication to wait for `validate-fast` and `validate-long`, run only on trusted `main` pushes, and happen before any GHCR publication
- [ ] GREEN: implement the smallest truthful Quay-first native publish flow for the three images with full-SHA tags only
- [ ] REFACTOR: keep build-target metadata in the existing target/image contracts instead of creating another registry-specific target registry

### Slice 4: Secret Handling And Redaction

- [ ] RED: add one failing contract that requires Quay credentials to stay out of validation jobs, use stdin login, and mask derived sensitive values before diagnostics
- [ ] GREEN: add the smallest truthful Quay login and masking steps using `QUAY_ROBOT_USERNAME` and `QUAY_ROBOT_PASSWORD`
- [ ] REFACTOR: keep secret-handling assertions centralized in `GithubWorkflowContract`

### Slice 5: Quay Vulnerability Gate

- [ ] RED: add one failing contract that requires a distinct Quay security gate to block every later registry push when Quay reports vulnerabilities or cannot produce a trusted result
- [ ] GREEN: implement the narrowest honest Quay security polling/wait flow supported by hosted behavior
- [ ] REFACTOR: keep the gate logic in one owned workflow step instead of scattered shell snippets

### Slice 6: GHCR Fan-Out After Quay Passes

- [ ] RED: add one failing contract that requires GHCR publication to depend on the Quay gate
- [ ] GREEN: implement the smallest truthful downstream GHCR publication path after the Quay pass signal exists
- [ ] REFACTOR: keep GHCR as a downstream fan-out boundary instead of a second independent publish graph

### Slice 7: Hosted Verification And Final Boundary Pass

- [ ] RED: if a real hosted `publish-images` run on `main` does not prove masked Quay diagnostics, Quay security gating, and GHCR-after-Quay ordering, the task is not done
- [ ] GREEN: inspect the hosted run/logs with the existing authenticated workflow-debug path and record concrete evidence in the task file
- [ ] REFACTOR: run one final `improve-code-boundaries` pass so publication coordinates, runtime artifact identity, and workflow security ownership stay clean

## Execution Guardrails

- Do not read, print, or otherwise expose secret values to discover Quay coordinates.
- Do not switch this plan back to `TO BE VERIFIED` unless hosted reality exposes a new interface or ownership question that the current plan cannot answer honestly.
- Do not run `make test-long` locally for this planning turn.
- During execution, run `make check`, `make lint`, and `make test` before any task completion bookkeeping.
- If hosted reality disproves the chosen Quay API/security-gate behavior, switch back to `TO BE VERIFIED` immediately instead of faking the gate.

## Next Step After Resume

- Start TDD execution from Slice 1 using the hosted-verified Quay boundary:
  - `QUAY_REGISTRY=quay.io`
  - `QUAY_NAMESPACE=determined_keldysh`
- Use the hosted `401 UNAUTHORIZED` push failure as the first concrete red signal.
  - preserve the explicit namespace boundary
  - fix the workflow and contracts so the Quay-first publish path is authorized, gated, and then re-verified online on `main`
- Keep final hosted verification targeted at `refs/heads/main` specifically, because connected repository metadata still reports `master` as the default branch on 2026-04-20.

Plan path: `.ralph/tasks/story-21-github-workflows-image-publish/06-task-publish-the-three-image-split-to-quay-with-strict-secret-redaction-and-main-only-access_plans/2026-04-20-explicit-quay-namespace-boundary-plan.md`

NOW EXECUTE
