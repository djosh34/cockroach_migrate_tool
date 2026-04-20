# Plan: Fix The Hosted Quay Security Gate And Parallelize Raw Image Build Work Without Weakening Release Gates

## References

- Task:
  - `.ralph/tasks/story-26-hosted-workflow-failure-investigation/01-task-debug-hosted-github-workflow-failures-parallelize-image-builds-and-surface-quay-security-findings.md`
- Workflow under change:
  - `.github/workflows/publish-images.yml`
  - `.github/workflows/AGENTS.md`
- Operator-facing registry surface that may need small truth-fixing edits if the workflow summary changes:
  - `README.md`
- Hosted verification path:
  - `github-api-auth-wrapper`
  - `/home/joshazimullah.linux/github-api-curl`
- Skills required during execution:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This turn is planning-only because the task had no plan path yet.
- Workflow-specific repo instructions override the generic local-test default for this task.
  - `.github/workflows/AGENTS.md` explicitly says not to invent local workflow tests and not to use `make check`, `make lint`, `make test`, or `make test-long` as proof for GitHub workflow behavior.
  - Execution verification for this task must therefore be hosted red/green using authenticated GitHub API log inspection on the latest `master` run.
- The active repository is `djosh34/cockroach_migrate_tool`.
- The latest workflow reality must be re-checked first on resume, because run `#37` was still in progress during planning.
- No error path may be swallowed.
  - If execution finds a distinct defect that cannot be fixed within this task, create a bug immediately with `add-bug`.

## Current Hosted Evidence Captured On 2026-04-20

- `publish-images` is the only active GitHub Actions workflow for this repo.
- Recent hosted behavior is consistently red on `master`.
  - Runs `#28` through `#36` all concluded `failure`.
  - Run `#37` for commit `907679b6cd69e7d97a63e074406b85d9efd1ff1c` was still `in_progress` while planning.
- The latest completed failed run inspected was `#36` for commit `4fa0b518d064e50720655ba77581df58dd9a8aee`.
  - `validate-fast` succeeded.
  - `validate-long` succeeded.
  - All six `publish-image` matrix jobs succeeded.
  - `quay-security-gate` failed immediately.
  - `publish-manifest` was skipped.
- Downloaded published image-ref artifacts from run `#36` prove the raw Quay publish already happened before the failure.
  - Example refs:
    - `quay.io/cockroach_migrate_tool/runner:4fa0b518d064e50720655ba77581df58dd9a8aee-amd64`
    - `quay.io/cockroach_migrate_tool/verify:4fa0b518d064e50720655ba77581df58dd9a8aee-amd64`
- The failing step is narrower than the old story-21 assumptions.
  - The first Quay tag lookup succeeds.
  - The workflow then aborts on `test "${is_manifest_list}" = "false"`.
  - Public Quay API inspection for the pushed runner tag shows:
    - the tag exists
    - `is_manifest_list=true`
    - `child_manifest_count=2`
  - Public Quay API inspection for the returned manifest digest shows:
    - the security endpoint exists
    - it returned `status=queued`
- Current root cause from real hosted evidence:
  - the gate wrongly assumes each per-platform pushed tag resolves to a non-manifest-list image
  - the job fails before it can print scanner status, vulnerability findings, or a clear operator-facing explanation

## Improve-Code-Boundaries Focus

- Primary boundary smell:
  - `.github/workflows/publish-images.yml` currently conflates three different responsibilities into one muddy release path:
    - raw build-and-push work
    - Quay scan interpretation
    - downstream release/promotion publication
- Required cleanup during execution:
  - keep raw image build/push work as one boundary
  - keep Quay scan polling and result classification as one boundary
  - keep release/promotion publication as one boundary
  - add one explicit operator-facing summary boundary so "published already" and "security failed" are not conflated
- Smells to avoid:
  - scattering Quay API parsing across multiple shell fragments
  - adding local Rust tests that pretend to validate hosted workflow behavior
  - duplicating the same status-classification logic in multiple jobs
  - weakening release gates just to make the workflow green

## Public Behavior To Establish

- The latest hosted workflow evidence must prove whether `validate-long` is failing or whether another stage is the real blocker.
  - Current evidence already points to `quay-security-gate`, not `validate-long`.
- Raw image build work must overlap with `validate-fast` and `validate-long` where technically possible.
- Publish/promotion decisions must remain gated on successful validation lanes.
- The Quay security stage must classify failures clearly:
  - vulnerabilities found
  - scanner or API failure
  - timeout or never-scanned state
  - unexpected response shape
- Workflow logs or summaries must make it explicit when raw Quay image publication succeeded before the security gate failed.
- If the Quay API reports findings, the workflow must surface enough detail for an operator to see what failed.

## Files Expected To Change During Execution

- [ ] `.github/workflows/publish-images.yml`
  - fix the Quay gate’s wrong manifest-list assumption
  - parallelize raw build work with validation
  - preserve honest release gating
  - add explicit failure/result surfacing
- [ ] `README.md`
  - only if a small truth-fixing note is needed so the operator-facing CI/publish story matches the workflow behavior
- [x] `.ralph/tasks/story-26-hosted-workflow-failure-investigation/01-task-debug-hosted-github-workflow-failures-parallelize-image-builds-and-surface-quay-security-findings.md`
  - mark acceptance evidence only after hosted verification succeeds
- [x] No local workflow test files should be added
  - `.github/workflows/AGENTS.md` forbids fake local workflow testing for this work

## TDD Execution Order

### Slice 0: Re-Verify The Latest Hosted Run First

- [x] RED: inspect the latest `publish-images` run on `master` with `/home/joshazimullah.linux/github-api-curl`
- [x] GREEN: confirm whether the active failure is still the Quay gate manifest-list assumption or whether a newer hosted failure changed the problem
- [x] REFACTOR: if the latest hosted reality changed materially, switch this plan back to `TO BE VERIFIED` instead of forcing the old fix

### Slice 1: Fix The Real Quay Gate Failure Mode

- [x] RED: use the latest hosted failed run to prove the gate still exits before surfacing scan status because it assumes `is_manifest_list=false`
- [x] GREEN: make the smallest workflow change that treats the returned Quay digest honestly
  - either poll the manifest-list digest directly when Quay supports it
  - or descend into child manifests deliberately when that is the truthful security boundary
  - do not keep the brittle `test "${is_manifest_list}" = "false"` assumption
- [x] REFACTOR: centralize the Quay API/tag/security parsing inside one owned gate step so result classification is not duplicated

### Slice 2: Surface Actionable Quay Security Outcomes

- [x] RED: prove from hosted logs that the workflow currently collapses every gate failure into opaque `exit 1`
- [x] GREEN: update the gate to emit explicit branches for:
  - vulnerabilities found
  - scanner/API/auth failure
  - queued/scanning timeout
  - unexpected response payload
- [x] REFACTOR: keep the output vocabulary tight and operator-facing so future failures are diagnosable from logs alone

### Slice 3: Parallelize Raw Build Work Without Weakening Release Gates

- [x] RED: prove from the current workflow graph that raw image build work still waits for both validation jobs before starting
- [x] GREEN: restructure the workflow so raw Quay build/push work starts in parallel with `validate-fast` and `validate-long`
  - preserve the rule that release/promotion jobs do not proceed until validations pass
  - keep `quay-security-gate` and any GHCR/promotion work gated on the required validation lanes plus successful raw build completion
- [x] REFACTOR: keep the workflow graph honest by separating raw build concurrency from release decisions rather than mixing them in one serial job chain

### Slice 4: Make Published-State Versus Scan-Failure State Explicit

- [x] RED: prove from hosted run output that an operator cannot currently tell cleanly that image publication already succeeded before the security gate failed
- [x] GREEN: add the smallest truthful summary/logging path that records:
  - which raw Quay refs were published
  - whether validation passed
  - whether the security gate failed
  - whether downstream manifest/GHCR promotion was skipped
- [x] REFACTOR: keep this summary in one explicit reporting boundary instead of repeating status prints in multiple jobs

### Slice 5: Hosted Verification To Green

- [x] RED: the task is not done until a real hosted `publish-images` run on `master` proves:
  - the latest real blocker is fixed
  - validation and raw build work overlap where intended
  - release/promotion remains gated
  - Quay scan failures are classified clearly
  - published-before-scan-failure state is explicit when relevant
- [x] GREEN: use authenticated GitHub API access to inspect the hosted verification run and record concrete evidence in the task file
- [x] REFACTOR: do one final `improve-code-boundaries` pass on the workflow so raw build, scan interpretation, and promotion/reporting responsibilities stay cleanly separated

## Execution Guardrails

- Do not create local Rust tests or local workflow test harnesses for this task.
- Do not run `make check`, `make lint`, `make test`, or `make test-long` as proof of workflow correctness for this workflow-only task.
- Do not weaken the validation gate merely to start build work earlier.
- Do not hide Quay API failures behind generic exit codes.
- Do not assume the current Quay manifest-list shape is accidental; use the hosted evidence and Quay API behavior as the source of truth.
- If Quay’s API cannot truthfully expose vulnerability findings for the relevant digest boundary, file a bug immediately instead of papering over the limitation.

## Next Step On Resume

- Start with Slice 0 against the latest hosted `master` run.
- If the manifest-list failure is still current, implement the narrowest fix in `.github/workflows/publish-images.yml`, then verify online again.
- Only after hosted verification is green should the task file be updated to passed and the Ralph completion flow continue.

Plan path: `.ralph/tasks/story-26-hosted-workflow-failure-investigation/01-task-debug-hosted-github-workflow-failures-parallelize-image-builds-and-surface-quay-security-findings_plans/2026-04-20-hosted-quay-gate-and-parallel-build-plan.md`

NOW EXECUTE
