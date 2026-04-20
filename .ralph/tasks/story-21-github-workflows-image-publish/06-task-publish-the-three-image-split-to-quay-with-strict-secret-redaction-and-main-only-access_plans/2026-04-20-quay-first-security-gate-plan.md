# Plan: Publish The Three-Image Split To Quay First With A Required Security Gate

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
- Hosted publish/debug evidence path already established by story 21 task 05:
  - `.ralph/tasks/story-21-github-workflows-image-publish/05-task-debug-real-github-image-build-failures-using-authenticated-workflow-api-log-access.md`
  - `.ralph/tasks/story-21-github-workflows-image-publish/05-task-debug-real-github-image-build-failures-using-authenticated-workflow-api-log-access_plans/2026-04-20-real-github-image-failure-debug-plan.md`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient approval for the interface direction and test priorities in this turn.
- This turn is planning-only because task 06 had no plan file or execution marker.
- The current `publish-images` workflow is already green for the GHCR-only path.
  - Execution must preserve the existing trusted-trigger, validation, native-platform, and artifact guarantees while inserting Quay-first publication and security gating.
- Quay must become the canonical first publish boundary.
  - No GHCR push may happen until the Quay publish and Quay vulnerability gate have both passed.
- The Quay namespace/repository coordinates must be expressed through non-secret metadata.
  - Do not derive the Quay namespace by printing or otherwise reading the robot secret value.
  - If execution cannot establish honest non-secret Quay coordinates from repo-owned configuration or public repository metadata, switch this plan back to `TO BE VERIFIED`.
- Execution must confirm the supported Quay vulnerability-check path from real hosted behavior and official product behavior before finalizing the poll/gate implementation.
  - If the assumed Quay scan API or polling semantics are wrong, switch back to `TO BE VERIFIED` instead of faking a gate.
- Preferred publish topology is:
  - build and push native platform images once to Quay
  - assemble canonical Quay multi-arch SHA tags
  - wait for Quay security results and fail on findings
  - only then fan out the already-scanned Quay refs into GHCR
- If cross-registry fan-out from Quay refs to GHCR proves unsupported or materially changes the published artifact in hosted practice, switch back to `TO BE VERIFIED`.
- `make test-long` is not a default end-of-task lane here.
  - Only run it if execution changes ultra-long-lane selection or proves that this task now touches that boundary.
- If any secret value is actually read during implementation or hosted debugging, add a rotation-warning report immediately under `.ralph/reports` and stop treating the task as routine.

## Current State Summary

- `.github/workflows/publish-images.yml` currently models one registry-specific publish path:
  - validate fast
  - validate long
  - publish native per-platform refs to GHCR
  - assemble the final GHCR multi-arch manifests
- `GithubWorkflowContract` already owns the public workflow guarantees for:
  - trusted triggers
  - validation-before-publish ordering
  - least-privilege permissions
  - native `amd64` and `arm64` publication
  - SHA-only tags
  - masked derived credentials
  - source-controlled compose artifact publication
- The current workflow already uses direct shell steps rather than third-party login/build wrappers, which matches the task requirements.
- The main boundary smell is not missing tests. It is wrong ownership:
  - `PublishedRuntimeArtifactContract::registry_host()` currently says the published runtime artifacts "are GHCR"
  - that is false for this task, because image identity and registry fan-out are separate concerns
  - if Quay is added on top of that shape, registry knowledge will sprawl across the workflow, README, and test support as duplicated raw strings
- There is currently no Quay publish path, no Quay vulnerability gate, and no real hosted proof that Quay masking/redaction works.
- There is also no explicit non-secret workflow boundary for Quay host/namespace/repository naming.
- Hosted execution on commit `06860f7ebf5d14a0cd2bf47143ad4948b934e14f` disproved the key namespace assumption in this plan:
  - `validate-fast` and `validate-long` both passed
  - Quay login succeeded and masked diagnostics stayed redacted
  - Quay publish lanes then failed while pushing to `quay.io/djosh34/...` with `401 UNAUTHORIZED` on blob `HEAD` requests
  - this means `github.repository_owner` is not a truthful Quay publication boundary for this repository, so Quay coordinates and/or permissions must be re-planned from a non-secret source before execution resumes

## Public Contract To Establish

- One contract fails if the workflow no longer publishes the canonical three-image set to Quay after `validate-fast` and `validate-long`.
- One contract fails if Quay publish stops using exact full-commit-SHA tags for the automated path.
- One contract fails if GHCR publication can start before the Quay security gate passes.
- One contract fails if Quay secrets are visible to validation jobs, PR-like triggers, or any non-`main` push path.
- One contract fails if Quay credentials are passed on shell command lines instead of via stdin or supported environment mechanisms.
- One contract fails if derived non-secret sensitive values are not explicitly masked with `::add-mask::` before diagnostic output.
- One contract fails if the workflow no longer proves Quay-first ordering and GHCR-later fan-out in one honest topology.
- One contract fails if the repo still treats registry host as part of the published runtime artifact identity instead of part of the workflow/publication boundary.
- One contract fails if hosted main-push logs do not provide concrete evidence that:
  - Quay secrets stayed redacted
  - Quay security scanning ran
  - GHCR publication happened only after the Quay gate passed

## Improve-Code-Boundaries Focus

- Primary smell:
  - registry choice is currently living in the wrong place
  - `PublishedRuntimeArtifactContract` should own the canonical image set and operator artifact names, not whether CI publishes those artifacts to GHCR, Quay, or both
- Required cleanup during execution:
  - remove or relocate the GHCR-only `registry_host()` ownership from the public runtime artifact contract
  - keep registry/security workflow assertions in `GithubWorkflowContract`, where publish topology already lives
  - introduce at most one honest, repo-owned publication metadata boundary for registry-specific coordinates if execution needs one
- Preferred cleanup shape:
  - published runtime artifact spec remains registry-agnostic
  - workflow contract owns Quay/GHCR topology, ordering, secret boundaries, and SHA-tag publication rules
  - README keeps only the operator-facing registry coordinates that remain part of the public pull contract
- Smells to avoid:
  - duplicating Quay and GHCR coordinates in README, workflow, and tests with no shared owner
  - creating a second ad hoc shell-helper layer just to hide one-off registry string manipulation
  - rebuilding the same image twice in two jobs if the Quay-first topology can honestly fan out from already-published refs
  - reading the robot secret to discover namespace or repository naming

## Interface And Topology Decisions

- Keep one canonical workflow:
  - `.github/workflows/publish-images.yml`
- Keep `GithubWorkflowContract` as the single owner of workflow/publication behavior assertions.
  - Do not create a second YAML parser or a Quay-only contract file with overlapping concerns.
- Keep `PublishedRuntimeArtifactContract` focused on image identity and operator artifacts.
  - Registry host must not stay there after this task.
- Preferred execution topology:
  - `validate-fast`
  - `validate-long`
  - Quay per-platform publish job family
  - Quay manifest assembly job
  - Quay vulnerability/security gate job
  - GHCR fan-out job that consumes the already-scanned Quay refs
- The GHCR path should preferably copy from Quay refs rather than rebuild.
  - If the hosted reality disproves that design, switch back to `TO BE VERIFIED`.
- The Quay host/namespace/repository mapping should be explicit and non-secret.
  - Prefer workflow-level env or one repo-owned metadata boundary.
  - Do not smuggle it through secrets.
- README quick-start image pulls should stay on the existing operator-facing registry unless execution proves the public contract changed.
  - This task is about CI publication topology and security, not a broad operator registry strategy change.

## Files And Structure To Add Or Change

- [x] `.github/workflows/publish-images.yml`
  - add Quay-first publication and required security gating
  - preserve existing trusted-trigger and validation boundaries
- [x] `crates/runner/tests/support/github_workflow_contract.rs`
  - add or update the workflow/publication assertions for:
    - Quay-first ordering
    - Quay secret scoping
    - SHA-only Quay tags
    - security-gate-before-GHCR behavior
    - explicit masking and no command-line secret leakage
- [x] `crates/runner/tests/ci_contract.rs`
  - add the behavior-level failing slices that exercise the new public workflow guarantees
- [x] `crates/runner/tests/support/published_runtime_artifact_contract.rs`
  - remove or relocate the GHCR-only registry-host ownership so the runtime artifact contract stops lying about publication topology
- [x] `crates/runner/tests/support/published_image_contract.rs`
  - update only if it still leaks registry-specific assumptions after the boundary cleanup
- [x] `README.md`
  - update the CI publish safety section if needed so reviewers can see:
    - Quay is first
    - GHCR is downstream
    - security/redaction behavior is intentional
  - avoid unnecessary changes to the operator quick-start unless the public pull contract truly changes
- [x] `.ralph/tasks/story-21-github-workflows-image-publish/06-task-publish-the-three-image-split-to-quay-with-strict-secret-redaction-and-main-only-access.md`
  - add execution evidence and final acceptance status only after hosted verification succeeds
- [x] No long-lived debug transcript or secret-derived config file is expected by default
  - only add durable files if the workflow contract cannot honestly express the needed boundary

## TDD Execution Order

### Slice 1: Tracer Bullet For Registry Boundary Ownership

- [x] RED: add one failing contract that proves published runtime artifact identity is registry-agnostic and that workflow publication topology, not the artifact contract, owns Quay-vs-GHCR behavior
- [x] GREEN: remove or relocate the GHCR-only `registry_host()` assumption from the public runtime artifact contract and adjust the smallest truthful callers
- [x] REFACTOR: collapse raw registry host assumptions so execution does not have to thread GHCR/Quay knowledge through unrelated public artifact helpers

### Slice 2: Quay-First Publish Topology

- [x] RED: add one failing workflow contract that requires Quay publication to happen only after `validate-fast` and `validate-long`, only on pushes to `main`, and before any GHCR publication path
- [x] GREEN: add the smallest honest Quay publish topology with native `amd64` and `arm64` lanes, explicit non-secret Quay coordinates, and full-SHA tags only
- [x] REFACTOR: keep image/build metadata coming from existing build-target/public-artifact boundaries instead of inventing a separate Quay target registry

### Slice 3: Secret Handling And Redaction Boundary

- [x] RED: add one failing contract that requires Quay credentials to stay out of validation jobs, avoid command-line password passing, and explicitly mask any derived sensitive values before log output
- [x] GREEN: implement the smallest truthful login/masking flow for Quay using `QUAY_ROBOT_USERNAME` and `QUAY_ROBOT_PASSWORD`
- [x] REFACTOR: keep secret-handling assertions inside `GithubWorkflowContract` rather than scattering raw YAML string checks through unrelated tests

### Slice 4: Quay Vulnerability Gate

- [x] RED: add one failing workflow contract that requires a distinct Quay security gate to block every later registry push when Quay reports vulnerabilities or fails to produce a trusted result
- [x] GREEN: implement the narrowest honest Quay scan wait/poll step that fails loudly on findings and on missing/indeterminate scan results
- [x] REFACTOR: keep Quay gate parsing inside one owned workflow step; do not grow a pile of single-use shell helpers

### Slice 5: GHCR Fan-Out After Quay Passes

- [x] RED: add one failing workflow contract that requires GHCR publication to depend on the Quay gate and forbids any downstream GHCR push before the Quay pass signal exists
- [x] GREEN: publish the GHCR refs from the already-scanned Quay refs if hosted behavior supports it; otherwise switch back to `TO BE VERIFIED`
- [x] REFACTOR: keep the GHCR path a downstream fan-out boundary, not a second independent build graph

### Slice 6: Hosted Verification And Final Boundary Pass

- [x] RED: if a real hosted `publish-images` run on `main` does not prove masked Quay diagnostics, Quay gate execution, and GHCR-after-Quay ordering, the task is not done
- [x] GREEN: inspect the hosted run/logs with the existing authenticated workflow-debug path from task 05, update the task outcome with concrete evidence, and keep the CI publish safety docs honest
- [x] REFACTOR: run one final `improve-code-boundaries` pass so registry identity, artifact identity, and workflow security ownership are not muddied after the Quay work lands

## TDD Guardrails For Execution

- One failing slice at a time.
- Do not bulk-edit the whole workflow before the first failing contract exists.
- Do not invent a fake Quay gate by checking for step success alone; the gate must be tied to Quay vulnerability results.
- Do not read, print, or otherwise expose secret values in order to discover namespace, debug login, or prove masking.
- Do not leave GHCR publication as a peer path that can run without Quay success.
- Do not keep `PublishedRuntimeArtifactContract` GHCR-specific after the refactor.
- If the hosted Quay API/scan behavior, namespace ownership, or registry-copy topology proves materially different from this plan, switch back to `TO BE VERIFIED` immediately.

## Re-Planning Trigger Captured

- Trigger observed on 2026-04-20 during hosted run `publish-images` #15 for commit `06860f7ebf5d14a0cd2bf47143ad4948b934e14f`.
- Concrete failure:
  - the Quay robot credentials authenticate successfully
  - the publish lanes fail when pushing `quay.io/djosh34/cockroach-migrate-verify:*` and sibling refs because Quay returns `401 UNAUTHORIZED` on blob `HEAD` requests
- Required next planning question:
  - what non-secret repository-owned source should define the real Quay namespace/repository coordinates, or what existing Quay repository permission/bootstrap step is missing?

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] Do not run `make test-long` unless execution changes ultra-long-lane selection or proves this task now touches that boundary
- [x] Real hosted `publish-images` main-push verification that proves:
  - Quay login diagnostics stayed redacted
  - Quay vulnerability gating ran and passed
  - GHCR publication started only after the Quay gate passed
- [x] One final `improve-code-boundaries` pass after local lanes are green
- [x] Update the task file acceptance checkboxes, set `<passes>true</passes>`, run `.ralph/task_switch.sh`, commit, and push only after all required evidence exists

Plan path: `.ralph/tasks/story-21-github-workflows-image-publish/06-task-publish-the-three-image-split-to-quay-with-strict-secret-redaction-and-main-only-access_plans/2026-04-20-quay-first-security-gate-plan.md`

TO BE VERIFIED
