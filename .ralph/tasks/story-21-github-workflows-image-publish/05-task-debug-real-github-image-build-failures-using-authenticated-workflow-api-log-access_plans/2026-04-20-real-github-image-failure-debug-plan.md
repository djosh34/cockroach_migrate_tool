# Plan: Debug Real GitHub Image-Build Failures With Authenticated Hosted Evidence

## References

- Task:
  - `.ralph/tasks/story-21-github-workflows-image-publish/05-task-debug-real-github-image-build-failures-using-authenticated-workflow-api-log-access.md`
- Current workflow and workflow-contract boundary:
  - `.github/workflows/publish-images.yml`
  - `crates/runner/tests/ci_contract.rs`
  - `crates/runner/tests/support/github_workflow_contract.rs`
  - `crates/runner/tests/support/image_build_target_contract.rs`
- Current published runtime/publication boundaries:
  - `crates/runner/tests/support/published_image_contract.rs`
  - `crates/runner/tests/support/published_runtime_artifact_contract.rs`
  - `crates/runner/tests/support/compose_artifact_contract.rs`
- Current operator-facing documentation:
  - `README.md`
- Authenticated GitHub API access path:
  - `/home/joshazimullah.linux/github-api-curl`
- Skills:
  - `tdd`
  - `improve-code-boundaries`
  - `github-api-auth-wrapper`

## Planning Assumptions

- The task markdown is sufficient approval for the interface direction and test priorities in this turn.
- This turn is planning-only because task 05 had no existing plan path or execution marker.
- Hosted GitHub evidence is the real source of truth for this task.
  - Local reasoning, local workflow contracts, and local dry-runs are useful only if they lead to a hosted run diagnosis that matches reality.
  - If hosted workflow evidence contradicts the local workflow-contract model, the hosted evidence wins.
- The execution turn must use `/home/joshazimullah.linux/github-api-curl` for authenticated GitHub API access rather than reading or exposing any token.
- The current local workflow/test boundary already encodes many desired invariants:
  - `push` to `master` only
  - explicit validation before publish
  - native `amd64` and `arm64` publication lanes
  - manifest fan-in after per-platform publication
  - masked derived credential diagnostics
  - compose artifact upload
- This task is still necessary even if the current head already passes in hosted CI.
  - The execution turn must inspect real hosted failures, identify the actual failure modes that existed, and ensure the current workflow/test/doc boundaries reflect those real causes rather than local guesses.
  - If the latest hosted run is already green, execution should inspect the most recent failing runs in the same workflow history before deciding whether the fixes are already complete.
- `make test-long` is not a default end-of-task lane here.
  - Only run it if execution changes ultra-long-test selection or the task evidence proves it is required.
- If the first hosted inspection shows the real failure mode needs a materially different workflow/public contract than planned below, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Current State Summary

- `.github/workflows/publish-images.yml` already has explicit validation and trusted publish boundaries:
  - `validate-fast`
  - `validate-long`
  - `publish-image`
  - `publish-manifest`
- The workflow already prints a masked diagnostic for derived registry credentials in both publish jobs.
- The workflow already uploads:
  - per-image/per-platform publication artifacts
  - `published-image-manifest`
  - `published-compose-artifacts`
- `crates/runner/tests/support/github_workflow_contract.rs` already owns the public workflow boundary for:
  - trusted triggers
  - permission isolation
  - validation-before-publish ordering
  - native arm64 lane assertions
  - manifest fan-in
  - credential masking expectations
  - compose artifact publication
- What is still missing for task 05 is not basic local workflow shape but evidence-driven closure:
  - inspect real failing hosted runs rather than inferring causes from YAML
  - capture the actual failure causes, including platform-specific ones
  - tighten local contracts only where hosted evidence exposed missing protection
  - record the real failure modes fixed instead of leaving the task at the level of abstract CI safety claims
- Boundary smell from `improve-code-boundaries`:
  - the repo currently has strong local contracts for workflow topology, but no single repo-owned boundary for authenticated hosted-run inspection and summarization
  - without care, task 05 could add a second pile of ad hoc shell commands, copied API URLs, and string parsing spread across notes, docs, and tests

## Interface And Boundary Decisions

- Keep one canonical workflow:
  - `.github/workflows/publish-images.yml`
- Keep `crates/runner/tests/ci_contract.rs` thin and behavior-focused.
  - It should continue to describe public workflow guarantees, not raw GitHub API payload parsing.
- Keep `GithubWorkflowContract` as the single owner of local workflow topology and YAML-boundary assertions.
  - Do not duplicate those invariants in a new hosted-log parser.
- Add at most one narrow hosted-evidence boundary if execution needs durable local coverage for a failure mode discovered in GitHub Actions logs.
  - Preferred direction:
    - keep raw hosted API inspection as execution-time evidence gathering
    - encode only the durable lesson learned into local workflow/readme contracts
  - Avoid:
    - checking in brittle snapshots of GitHub API JSON payloads
    - adding a permanent local parser for transient log text when a tighter workflow contract would express the same invariant honestly
- Use `/home/joshazimullah.linux/github-api-curl` exactly like `curl`, with all arguments passed through unchanged.
  - Do not read, print, request, or expose the token.
- Hosted debugging flow should follow one honest evidence path:
  - list recent `publish-images` workflow runs
  - identify the newest failing or relevant run for the current branch/commit history
  - inspect jobs and job conclusions
  - fetch the failing job logs
  - derive the real failure mode
  - add the smallest RED contract that would have caught that class of failure locally where practical
  - implement the smallest truthful fix
  - rerun local required lanes
  - inspect the next hosted run until the real publish path succeeds
- Treat architecture-specific failures as first-class.
  - The arm64 and amd64 lanes are separate public contracts now; execution must not compress them back into one generic "publish failed" conclusion.

## Public Contract To Establish

- One contract fails if the repo can no longer prove the trusted publish path is `push` to `master` only.
- One contract fails if the repo can no longer prove validation completes before publish starts.
- One contract fails if the repo can no longer prove the native `linux/amd64` and `linux/arm64` publish lanes remain distinct.
- One contract fails if the repo can no longer prove derived credentials are masked before diagnostic output.
- One contract fails if the repo can no longer prove compose artifacts and the published-image manifest ship through the canonical publish flow.
- One contract fails if a hosted-discovered failure mode remains expressible only through a tribal-knowledge debug note when it could be captured by a local workflow/test/doc contract.
- One contract fails if execution would need to inspect untrusted triggers, PR paths, or token exposure to debug the publish workflow.
- One contract fails if the task outcome cannot name the actual hosted failure modes fixed, including any platform-specific, secret-gating, or redaction issues discovered.

## Improve-Code-Boundaries Focus

- Primary smell:
  - local workflow intent is centralized, but hosted-debug knowledge is not yet centralized at all
- Required cleanup during execution:
  - do not add a permanent second source of truth for workflow topology outside `GithubWorkflowContract`
  - if hosted evidence exposes a real gap, encode the durable invariant once in the existing contract/helper boundary instead of scattering shell transcripts through docs/tests/task notes
  - keep task/task-plan outcome text factual and tied to specific hosted run evidence rather than vague "CI was debugged" summaries
- Smells to avoid:
  - pasting raw GitHub API payloads into checked-in files
  - creating a local JSON fixture hierarchy for transient workflow logs
  - duplicating workflow matrix metadata in ad hoc debug scripts
  - adding README/operator-facing text for contributor-only debug mechanics unless the hosted evidence proves the public contract itself was wrong

## Files And Structure To Add Or Change

- [x] `.github/workflows/publish-images.yml`
  - only if hosted evidence proves the current workflow still has a real failure
  - preserve trusted-trigger, permission, and validation boundaries while fixing the hosted failure honestly
- [x] `crates/runner/tests/support/github_workflow_contract.rs`
  - add or tighten the smallest contract needed to encode any durable hosted failure lesson
  - do not turn it into a GitHub API log parser
- [x] `crates/runner/tests/ci_contract.rs`
  - add the behavior-level failing slice only if a new durable workflow contract is needed
- [x] `README.md`
  - update only if hosted evidence shows the documented CI publish safety story is materially incomplete or wrong
- [x] `.ralph/tasks/story-21-github-workflows-image-publish/05-task-debug-real-github-image-build-failures-using-authenticated-workflow-api-log-access.md`
  - capture the real hosted failure modes fixed and final evidence only after execution succeeds
- [x] No new long-lived debug script or checked-in API transcript is expected by default
  - only add one if the hosted-debug path cannot stay honest without it

## TDD Execution Order

### Slice 1: Tracer Bullet For Authenticated Hosted Inspection

- [x] RED: use `/home/joshazimullah.linux/github-api-curl` to inspect the recent `publish-images` workflow history and identify one real failing or otherwise relevant hosted run instead of reasoning from local YAML alone
- [x] GREEN: establish the smallest repeatable hosted-inspection command path that can answer:
  - which run failed
  - which job failed
  - which platform/image lane failed
  - whether masking/redaction diagnostics appeared
- [x] REFACTOR: keep the command sequence concise and factual in the task outcome; do not create a sprawling repo-local debug subsystem

### Slice 2: First Durable Failure Contract

- [x] RED: from the first hosted failure, add one failing local contract only if the failure reveals a durable missing invariant that local tests should have caught
- [x] GREEN: implement the smallest truthful workflow/test/doc fix for that single failure mode
- [x] REFACTOR: encode the invariant in `GithubWorkflowContract` or the existing public contract owner rather than inventing a separate hosted-debug helper

### Slice 3: Repeat Hosted Evidence Loop Until Publish Succeeds

- [x] RED: trigger or inspect the next hosted `publish-images` run and treat every hosted failure as real RED, especially:
  - native `arm64` lane failure
  - native `amd64` lane failure
  - manifest assembly failure
  - artifact handoff failure
  - secret-gating failure
  - missing masking/redaction behavior
- [x] GREEN: fix only the current hosted failure and rerun the local required lanes before going back to hosted evidence
- [x] REFACTOR: after each hosted loop, collapse the lesson into the existing workflow contract boundary instead of accumulating one-off debug notes

### Slice 4: Secret Gating And Redaction Evidence

- [x] RED: use hosted logs and run metadata to confirm whether trusted-secret use is restricted to the intended `master` push path and whether masked diagnostics remain redacted in real logs
- [x] GREEN: if hosted evidence shows secrets/redaction drift, fix the workflow and add the narrowest local contract that prevents recurrence
- [x] REFACTOR: keep security assertions in the existing workflow-contract boundary and outcome summary, not in duplicated documentation prose

### Slice 5: Record The Actual Failure Modes Fixed

- [x] RED: if task 05 still cannot name the real hosted failure causes with run/job specificity, add the smallest missing evidence gathering needed before marking the task done
- [x] GREEN: update the task outcome and acceptance checkboxes only when the current hosted run succeeds and the task records the actual failure modes fixed, including architecture-specific findings where present
- [x] REFACTOR: ensure the final task text reflects concrete hosted evidence rather than speculative root causes

## TDD Guardrails For Execution

- One hosted failure slice at a time.
- Do not bulk-edit the workflow before the first hosted failure is identified.
- Do not treat local workflow contracts as sufficient proof for this task.
- Do not read or print the GitHub token; use `/home/joshazimullah.linux/github-api-curl`.
- Do not swallow hosted failures or "explain them away" without evidence.
- Do not overfit local tests to transient GitHub log wording when a more stable workflow invariant exists.
- If the real failure mode requires a different public contract or workflow shape than this plan assumes, switch back to `TO BE VERIFIED` immediately.

## Final Verification For The Execution Turn

- [x] Real hosted GitHub workflow runs and logs inspected through authenticated API access until the publish path succeeds
- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] Do not run `make test-long` unless execution changes ultra-long-lane selection or the task explicitly proves it is required
- [x] One final `improve-code-boundaries` pass after required lanes are green
- [x] Update the task file acceptance checkboxes, set `<passes>true</passes>`, and record the real hosted failure modes only after success

Plan path: `.ralph/tasks/story-21-github-workflows-image-publish/05-task-debug-real-github-image-build-failures-using-authenticated-workflow-api-log-access_plans/2026-04-20-real-github-image-failure-debug-plan.md`

NOW EXECUTE
