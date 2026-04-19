# Plan: CI Workflow Attack-Surface And Secret-Safety Sanity Checks

## References

- Task: `.ralph/tasks/story-15-ci-build-test-image-pipeline/02-task-add-ci-sanity-check-for-workflow-attack-vectors-and-secret-safety.md`
- Previous story-15 execution plan:
  - `.ralph/tasks/story-15-ci-build-test-image-pipeline/01-task-build-master-only-pipeline-for-full-tests-and-scratch-ghcr-image_plans/2026-04-19-master-only-ghcr-scratch-pipeline-plan.md`
- Existing workflow and workflow-contract files:
  - `.github/workflows/master-image.yml`
  - `crates/runner/tests/ci_contract.rs`
  - `crates/runner/tests/support/github_workflow_contract.rs`
- Existing image-publish contract support:
  - `crates/runner/tests/support/runner_docker_contract.rs`
  - `crates/runner/tests/long_lane.rs`
- Repository quality lanes:
  - `Makefile`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown is treated as approval for the public interface and behavior priorities in this turn.
- This turn is planning-only because the task file had no plan path and no existing execution plan for this task.
- The workflow from story-15 task 01 already provides a good first contract boundary, so this task should strengthen that boundary instead of adding a second parallel "security checker".
- The right public contract is still repository-owned tests over the checked-in workflow YAML, not tribal knowledge and not an external scanner service.
- The security contract should fail loudly on workflow drift. If a future edit adds an unsafe trigger, broadens permissions, or reintroduces an outsider-controlled publish path, tests must fail immediately.
- Defense in depth is required here. Trigger restrictions alone are not enough; the publish job should also carry explicit trusted-event guards and minimized permissions.
- If the first RED slice proves the existing workflow contract support cannot express these policy checks cleanly without scattering YAML parsing and string matching across multiple test files, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - random pull requests, forks, and outsider-controlled events cannot reach the protected publish path
  - workflow drift cannot silently add expensive or unsafe trigger paths
  - secrets and token write permissions stay unavailable to untrusted or validation-only paths
  - the published image still comes only from the validated trusted `master` push commit
- Next-priority behaviors to prove:
  - the workflow does not rely on reusable-workflow, manual-dispatch, workflow-run, or pull-request-target paths that could later widen the attack surface
  - checkout and publish steps do not quietly persist credentials or override refs in ways that weaken the trust boundary
- Lower-priority concerns:
  - cosmetic workflow cleanup
  - adding more workflows
  - general application-security work outside CI/image publishing

## Current Risks To Tighten

- `.github/workflows/master-image.yml` currently sets `packages: write` at the workflow level, which is broader than necessary and lets the validation job inherit publish capability.
- The workflow currently relies on the top-level trigger alone to protect publishing. This is a good baseline but weak as defense in depth if someone later broadens the trigger set.
- The current contract tests prove some trigger and tag rules, but they do not yet explicitly guard against:
  - `pull_request_target`
  - `workflow_dispatch`
  - `workflow_run`
  - `workflow_call`
  - `schedule`
  - tag-triggered execution
  - reusable-workflow publish indirection
  - checkout credential persistence
  - ref overrides that could build something other than the trusted pushed commit

## Interface And Boundary Decisions

- Keep the public product interface unchanged. This task is about workflow policy and repository-owned tests only.
- Keep `crates/runner/tests/ci_contract.rs` as the thin public test surface for workflow behavior.
- Extend `crates/runner/tests/support/github_workflow_contract.rs` into the single honest owner for workflow trigger, permission, secret-safety, and trusted-publish assertions.
- Do not introduce a second support module such as `github_workflow_security_contract.rs`. That would split one policy boundary into two weaker ones.
- Prefer typed helpers inside `GithubWorkflowContract` for:
  - allowed triggers
  - forbidden triggers
  - top-level and per-job permissions
  - checkout step options
  - publish step guards and tag/ref sources
- Keep image-runtime assertions in `RunnerDockerContract`; only workflow-origin and publish-safety checks belong in `GithubWorkflowContract`.

## Public Contract To Establish

- One fast contract fails if the workflow defines any outsider-controlled or drift-prone trigger beyond trusted `push` to `master`.
- One fast contract fails if the workflow allows tag pushes, PR events, `pull_request_target`, `workflow_dispatch`, `workflow_run`, `workflow_call`, or `schedule`.
- One fast contract fails if publish-capable permissions are granted outside the publish job.
- One fast contract fails if checkout persists credentials or checks out a ref other than the pushed trusted commit.
- One fast contract fails if the publish job lacks an explicit trusted-event gate in addition to the top-level trigger.
- One fast contract fails if the publish job can publish anything other than the GHCR image tagged by `${{ github.sha }}` from the repository root after validation succeeds.
- If needed for clarity, one small repository doc update should state why random PRs cannot publish the real image and which tests guard that invariant.

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - workflow security policy is currently on the edge of being split across ad hoc YAML string checks in tests and one generic contract helper
- Required cleanup during execution:
  - centralize all new workflow-policy parsing and assertions in `GithubWorkflowContract`
  - expose intention-revealing assertion methods instead of making `ci_contract.rs` inspect raw YAML shape directly
  - keep publish/image boundary ownership separate:
    - workflow trust and permissions in `GithubWorkflowContract`
    - runtime image shape in `RunnerDockerContract`
- Smells to avoid:
  - adding another workflow-support file with overlapping policy logic
  - scattering raw `serde_yaml` traversal across multiple tests
  - asserting secrets or permissions through brittle full-file substring searches when typed YAML accessors would be clearer

## Files And Structure To Add Or Change

- [x] `.github/workflows/master-image.yml`
  - tighten permissions to least privilege
  - add explicit publish-job trust guard
  - disable credential persistence on checkout if needed
  - keep publish rooted in the validated commit SHA only
- [x] `crates/runner/tests/ci_contract.rs`
  - extend the existing public workflow contract tests with attack-surface and secret-safety behaviors
- [x] `crates/runner/tests/support/github_workflow_contract.rs`
  - add typed helpers/assertions for forbidden triggers, permission scope, checkout hardening, and trusted publish gating
- [x] `README.md`
  - only if execution shows a short CI publish-safety note is needed to satisfy the documented-outcome acceptance criterion cleanly
- [x] No product code changes are expected
  - only real bug fixes are allowed if the RED slices expose one

## TDD Execution Order

### Slice 1: Tracer Bullet For Unsafe Trigger Expansion

- [x] RED: add one failing contract test that proves the workflow allows only `push` to `master` and explicitly rejects `pull_request`, `pull_request_target`, `workflow_dispatch`, `workflow_run`, `workflow_call`, `schedule`, and tag-triggered execution
- [x] GREEN: make the minimum workflow/support changes needed to satisfy that trigger policy
- [x] REFACTOR: move trigger and forbidden-event parsing behind typed `GithubWorkflowContract` helpers

### Slice 2: Prove Least-Privilege Permissions And Secret Containment

- [x] RED: add the next failing contract that requires:
  - workflow-level permissions stay read-only
  - only the publish job gets `packages: write`
  - the validate job does not inherit publish capability
  - checkout steps do not persist credentials
- [x] GREEN: tighten workflow permissions and checkout options with the smallest change set
- [x] REFACTOR: keep permission lookup and checkout-step assertions in one workflow-contract boundary

### Slice 3: Prove The Publish Path Is Trusted Even If Triggers Drift Later

- [x] RED: add a failing contract that requires the publish job to carry an explicit trusted-event/job gate and forbids ref overrides, reusable-workflow indirection, or publish tags derived from anything other than `${{ github.sha }}`
- [x] GREEN: add the minimal publish-job guardrails needed to satisfy the contract
- [x] REFACTOR: keep tag/ref/trust assertions intention-revealing instead of sprinkling raw string checks through the test file

### Slice 4: Document The Safety Model Only If The Tests Alone Are Not Enough

- [x] RED: if the acceptance criterion for documented outcome is not clearly satisfied by test names plus workflow comments, add one failing doc contract or README expectation for the CI publish-safety explanation
- [x] GREEN: add the smallest truthful documentation update needed
- [x] REFACTOR: keep the explanation short and aligned with the enforced tests so docs and behavior cannot drift

### Slice 5: Full Repository Lanes And Final Boundary Review

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so workflow attack-surface policy has one honest owner and no duplicate YAML parsing

## TDD Guardrails For Execution

- Every new behavior assertion must fail before supporting workflow changes are added. If an assertion already passes, replace it with the next uncovered behavior.
- Do not satisfy this task with comments or prose alone. Repository-owned tests must enforce the policy.
- Do not leave `packages: write` at workflow scope.
- Do not introduce `pull_request_target`, `workflow_dispatch`, `workflow_run`, `workflow_call`, `schedule`, or tag triggers as part of this task.
- Do not allow the publish job to depend on user-controlled inputs, refs, or manual parameters.
- Do not swallow YAML parse failures, missing-step failures, permission lookup failures, or Docker/workflow errors. Any such failure is task-relevant and must fail loudly.

## Boundary Review Checklist

- [x] `GithubWorkflowContract` is the single owner for workflow-policy assertions
- [x] `ci_contract.rs` stays thin and behavior-focused
- [x] Publish-capable permissions exist only where publishing actually occurs
- [x] Workflow tests cover outsider PRs, forks, untrusted events, and CPU-burn drift through trigger policy
- [x] Publish still points only at the validated trusted commit SHA on `master`
- [x] No error path is swallowed

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
