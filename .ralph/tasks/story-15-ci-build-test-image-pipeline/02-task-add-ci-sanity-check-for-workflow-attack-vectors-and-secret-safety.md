## Task: Add a CI sanity and security check task that audits workflow attack vectors, secret exposure, and untrusted PR behavior <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Add the separate sanity-checking work requested by the PO so the CI/publish workflow is reviewed against realistic abuse paths before it is trusted. This task must inspect all plausible attack vectors around the workflow, especially cases where random pull requests, forks, or misconfigured triggers could burn CPU, access secrets, or cause malicious or unreviewed code to be published into the real OCI image. The higher order goal is to prove that the release path is locked to trusted events and that workflow mismanagement cannot silently widen the attack surface later.

In scope:
- explicit review of workflow triggers and event types
- review of any branch, tag, path, reusable-workflow, or manual-dispatch paths that could unintentionally enable execution or publish behavior
- review of token and secret exposure in relation to pull requests, forks, and untrusted contributors
- review of package-publish permissions and whether they are scoped minimally
- review of whether published images are guaranteed to come from the tested trusted commit on `master`
- review of CPU-burn risk from accidental trigger expansion or outsider-controlled events
- codifying these checks as repository-owned sanity checks, tests, assertions, or documented guardrails that fail loudly when the workflow becomes unsafe

Out of scope:
- implementing unrelated hardening outside the CI/image-publish area
- general application security unrelated to the workflow

Decisions already made by the PO and required by this task:
- the sanity check must look for all possible attack vectors around this CI task
- the review must explicitly consider random people opening random PRs
- the review must focus on preventing CPU waste from unwanted execution
- the review must focus on preventing key leakage through workflow mistakes
- the review must focus on preventing bad code from ending up in the real Docker image

This task should assume the repository is greenfield and should fail loudly rather than accepting unsafe fallback behavior. If the audit finds any workflow pattern that cannot be proven safe, the task must tighten or remove it instead of documenting away the risk.

</description>


<acceptance_criteria>
- [x] Red/green TDD covers the sanity checks or assertions that guard the workflow against unsafe trigger and permission expansion
- [x] The repository has a concrete sanity-check mechanism that reviews workflow attack vectors instead of relying on tribal knowledge
- [x] The checks explicitly cover pull requests, forks, outsider-controlled events, secret availability, package-publish permissions, and trusted-source guarantees for the published image
- [x] The checks fail loudly if workflow configuration drifts into a state that could leak secrets, waste CPU on untrusted events, or publish an image from untrusted code
- [x] The final documented outcome makes clear why random PRs cannot trigger the protected publish path or influence the real published image
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-15-ci-build-test-image-pipeline/02-task-add-ci-sanity-check-for-workflow-attack-vectors-and-secret-safety_plans/2026-04-19-workflow-attack-safety-plan.md</plan>
