# Plan: Investigate Transient Cachix 502 Push Failures In The Publish Images Workflow

## References

- Task:
  - `.ralph/tasks/bugs/bug-investigate-transient-cachix-502-push-failures-in-publish-images-workflow.md`
- Current workflow and cache bootstrap boundary:
  - `.github/workflows/publish-images.yml`
  - `.github/actions/setup-nix-cachix/action.yml`
- Adjacent completed migration that introduced the current Cachix setup:
  - `.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/03-task-replace-magic-nix-cache-with-cachix.md`
  - `.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/03-task-replace-magic-nix-cache-with-cachix_plans/2026-04-28-cachix-workflow-replacement-plan.md`
- Workflow-facing publish boundaries that must stay separate from cache bootstrap unless evidence proves otherwise:
  - `scripts/ci/publish-ghcr-multiarch-from-archives.sh`
  - `scripts/ci/publish-quay-from-ghcr.sh`
- Hosted inspection entrypoints:
  - `/home/joshazimullah.linux/github-api-curl`
  - GitHub Actions run logs downloaded with authenticated API access
- Repo validation entrypoints:
  - `Makefile`
- Required skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This turn is planning-only because the task is still pre-execution and has no plan pointer yet.
- The task markdown already declares this work as a TDD exception.
  - Do not add brittle tests that assert workflow text or log strings in repo tests.
  - The public verification boundary here is a real hosted GitHub Actions run plus authenticated log inspection.
- The execution turn must still follow the TDD mindset even without adding repo tests.
  - RED means reproducing a real hosted failure mode or proving the hosted evidence is still ambiguous.
  - GREEN means the smallest workflow or action change eliminates that exact failure mode or proves the problem is service-side and outside workflow control.
  - REFACTOR means tightening boundaries so cache setup, cache-health detection, and publish orchestration are not mixed together.
- No backwards-compatibility work is needed.
  - If the current workflow boundary is muddy, collapse or replace it instead of preserving it.
- `make check`, `make lint`, and `make test` remain mandatory at the end even if the actual bug is external to application code.
- `make test-long` must not run for this bug unless execution proves the task is really finishing a larger story or the task text changes to require it.

## Current State Summary

- The only local Cachix integration point is `.github/actions/setup-nix-cachix/action.yml`.
  - It installs Nix with `cachix/install-nix-action@v31`.
  - It enables the `djosh34` cache through `cachix/cachix-action@v17`.
- The `Publish Images` workflow uses that composite action in the only jobs that build Nix store paths:
  - `nix flake check`
  - `build-images`
- The reported hosted run `25077991160` already proved basic configuration is correct enough for successful workflow completion:
  - Cachix was enabled with the expected cache name and URI.
  - The overall workflow stayed green.
  - The failing surface is specifically the post-step push phase of the Cachix action.
- That means the first execution question is not "is Cachix configured at all?"
  - It is "does the current action/workflow boundary give us enough control and enough reporting when Cachix push health degrades?"
- The likely failure classes are:
  - Cachix service-side transient availability problems returning `502 Bad Gateway`
  - action-level retry behavior that eventually gives up while the workflow still succeeds
  - multipart upload failures that leave some required store paths absent from the binary cache
  - workflow policy mismatch where cache-upload health should be promoted from noisy post-step logs into an explicit signal

## Improve-Code-Boundaries Focus

- Primary boundary problem to attack:
  - cache bootstrap and cache-health policy are currently fused into a third-party post-step hidden behind one composite action use site
- Why this is a boundary smell:
  - the workflow owns job orchestration
  - the local composite action owns Nix + Cachix setup inputs
  - but nobody in the repo owns explicit cache push health evaluation or missing-path reporting
  - that leaves an important operational failure mode trapped in opaque vendor logs
- Desired boundary after execution:
  - workflow YAML owns only orchestration and fail/pass policy
  - the local `setup-nix-cachix` action owns installation and authentication only
  - one explicit repo-owned boundary owns cache health interpretation if the workflow needs stronger guarantees
- Preferred direction if a workflow-side fix is required:
  - keep `.github/actions/setup-nix-cachix/action.yml` narrow
  - add a separate repo-owned script or step for cache-health evidence collection/reporting rather than bloating the setup action into a mixed-responsibility helper
- Smells to avoid:
  - stuffing post-run log parsing into the setup composite action
  - duplicating ad hoc Cachix troubleshooting logic across jobs
  - changing publish scripts for a problem that occurs before publish starts
  - tolerating `|| true`, swallowed upload failures, or undocumented "green but degraded" behavior

## Intended Public Contract After Execution

- We know whether run `25077991160` reflects:
  - a still-reproducible workflow issue
  - a `cachix/cachix-action` behavior gap
  - or a transient Cachix service outage that the workflow cannot directly prevent
- We know whether failed `multipart-nar` uploads actually leave required store paths missing from the `djosh34` cache.
- If the right fix is in-repo, the workflow makes cache push degradation explicit instead of hiding it in post-step noise.
- If the right outcome is "service-side transient only", the task records that clearly with hosted evidence and does not add fake local complexity.
- The cache bootstrap boundary remains small and focused rather than turning into a grab bag helper.

## Files And Structures Expected To Change

- [ ] `.ralph/tasks/bugs/bug-investigate-transient-cachix-502-push-failures-in-publish-images-workflow.md`
  - add hosted evidence notes, checkbox completion, and final pass state during execution
- [ ] `.github/workflows/publish-images.yml`
  - only if hosted evidence proves the workflow needs explicit cache-health reporting or fail policy
- [ ] `.github/actions/setup-nix-cachix/action.yml`
  - only if setup inputs or action configuration are actually part of the fix
- [ ] `scripts/ci/...`
  - no publish-script changes expected by default
- [ ] a new small CI helper script
  - only if execution needs a repo-owned boundary for cache-health detection/reporting

## Type And Interface Decisions

- Do not create repo tests that inspect workflow YAML as text.
- Do not create Rust/Go tests for hosted-log strings.
- Treat these as the execution interfaces instead:
  - a real `push`-triggered workflow run
  - authenticated workflow/job log download
  - if needed, direct Cachix cache inspection for specific store paths referenced by hosted failures
- Keep store-path verification concrete.
  - If logs show `Failed to push /nix/store/...`, execution must check those exact paths or exact output paths from the build closure rather than speaking abstractly about "some cache misses."
- If a repo-owned policy step is introduced, it must consume explicit artifacts or explicit API output and fail honestly on degraded cache health.

## Execution Slices For The Next Turn

### Slice 0: Reproduce Or Disprove The Bug On A Fresh Hosted Run

- [ ] RED:
  - inspect the existing authenticated logs for run `25077991160` with `/home/joshazimullah.linux/github-api-curl`
  - trigger or identify a fresh `push`-based hosted run on current `HEAD`
  - download the fresh logs and check whether the same `502 Bad Gateway` post-step failures still occur
- [ ] GREEN:
  - none yet; this slice is about getting to an honest present-tense failure state
- [ ] REFACTOR:
  - if the current evidence collection flow is scattered, write down the exact commands and log locations in task notes for the next slices
- Stop condition:
  - if no fresh hosted run can be obtained or authenticated logs cannot be inspected, switch back to `TO BE VERIFIED`

### Slice 1: Determine Whether Missing Cache Uploads Are Real Or Only Noisy Logs

- [ ] RED:
  - from the failing hosted logs, extract exact `/nix/store/...` paths that the Cachix post-step reported as failed uploads
  - inspect whether those paths are actually present or absent in Cachix after the run
  - compare against the build outputs/closures required by the completed jobs
- [ ] GREEN:
  - capture one definitive conclusion:
    - the paths were eventually present anyway
    - some paths were missing but non-critical
    - or required paths were missing from cache and the workflow silently accepted degraded cache state
- [ ] REFACTOR:
  - normalize the evidence into a small per-job table in the task notes instead of leaving it as scattered raw log snippets
- Stop condition:
  - if Cachix API/CLI evidence is too weak to determine path presence confidently, switch back to `TO BE VERIFIED`

### Slice 2: Assign Root Cause To The Right Boundary

- [ ] RED:
  - inspect `cachix/cachix-action` documentation and behavior relevant to upload retries, post-run failure semantics, and multipart upload handling
  - compare that behavior with what the hosted logs show
- [ ] GREEN:
  - classify the failure as one of:
    - workflow configuration problem
    - local composite-action boundary problem
    - upstream action behavior limitation
    - Cachix service-side availability problem
- [ ] REFACTOR:
  - remove any invalid local fix ideas that target the wrong layer
- Stop condition:
  - if the evidence points to multiple plausible root causes with no clear primary owner, switch back to `TO BE VERIFIED`

### Slice 3: Apply The Smallest Valid Workflow-Side Fix If Needed

- [ ] RED:
  - only if Slices 0-2 prove a repo-side gap remains, define the smallest observable policy change that should go red first
  - likely candidates:
    - fail the job when cache push degradation crosses a clear threshold
    - add an explicit reporting step that summarizes failed uploads and missing paths
    - reduce mixed responsibilities by moving health interpretation into a dedicated script/step
- [ ] GREEN:
  - implement only the smallest change needed to make the hosted behavior explicit and correct
  - keep `.github/actions/setup-nix-cachix/action.yml` limited to setup/auth unless evidence proves otherwise
- [ ] REFACTOR:
  - if a helper script is introduced, make it the single repo-owned cache-health boundary and remove duplication
- Stop condition:
  - if the only honest conclusion is "upstream/service-side problem with no safe workflow fix," do not force local complexity; document the conclusion and leave the workflow untouched

### Slice 4: Hosted Verification Of The Final State

- [ ] RED:
  - rerun the hosted workflow after any repo-side change
  - inspect logs again with authenticated access
- [ ] GREEN:
  - prove one of these clearly:
    - the transient `502` problem no longer occurs
    - the problem still occurs but now fails or reports explicitly as designed
    - the problem is service-side and no repo-side change is appropriate
- [ ] REFACTOR:
  - tighten task notes so the final state is easy to audit without rereading raw logs

### Slice 5: Final Required Validation And Boundary Review

- [ ] Run `make check`
- [ ] Run `make lint`
- [ ] Run `make test`
- [ ] Do not run `make test-long`
- [ ] Final `improve-code-boundaries` pass:
  - cache setup remains a narrow boundary
  - cache-health policy, if added, lives in one explicit repo-owned place
  - no ignored errors, fallback hacks, or swallowed upload failures remain

## Planned Validation Commands

- `/home/joshazimullah.linux/github-api-curl ...`
  - exact GitHub API calls to list runs, fetch jobs, and download logs for the target hosted run
- `make check`
- `make lint`
- `make test`

## Expected Outcome

- The team has a hard answer about whether transient Cachix `502` upload failures are still happening and whether they materially damage cache completeness.
- Any repo-side fix stays small and boundary-driven instead of smearing cache-health logic across workflow YAML and helper actions.
- If the real culprit is upstream or service-side, the task ends with evidence instead of local guesswork.

Plan path: `.ralph/tasks/bugs/bug-investigate-transient-cachix-502-push-failures-in-publish-images-workflow_plans/2026-04-28-cachix-502-push-failure-investigation-plan.md`

NOW EXECUTE
