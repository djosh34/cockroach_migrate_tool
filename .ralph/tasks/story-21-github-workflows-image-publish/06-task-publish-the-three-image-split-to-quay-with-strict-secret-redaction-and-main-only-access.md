## Task: Publish the three-image split to Quay with strict secret redaction and `main`-only secret access <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Add Quay publication for the verify, SQL-emitter, and runner images using the repository secrets `QUAY_ROBOT_USERNAME` and `QUAY_ROBOT_PASSWORD`, while treating secret handling and log redaction as high-risk security work. The higher order goal is to make Quay the first publish and vulnerability gate so no image reaches downstream registries unless it has already passed the Quay security checks.

In scope:
- publish the verify image to Quay
- publish the one-time SQL-emitter image to Quay
- publish the runner scratch image to Quay
- use the existing repository secrets `QUAY_ROBOT_USERNAME` and `QUAY_ROBOT_PASSWORD`
- ensure Quay publish follows the same `main`-only, post-test, full-SHA-tag policy as the GitHub Container Registry path
- make Quay publication happen before GitHub Container Registry publication
- use Quay vulnerability checking as a required gate
- fail the workflow on vulnerability findings and prevent any later image push when the Quay vulnerability gate fails
- manually install dependencies where practical rather than relying on random third-party actions
- add explicit redaction and masking steps for any sensitive values that are not automatically protected as GitHub secrets
- verify that Quay credentials are never echoed, leaked via command lines where avoidable, or exposed through workflow logs
- verify that no pull request, issue event, non-`main` branch push, forked workflow, or other unintended trigger can access or use the Quay secrets
- verify that log redaction/masking actually works in the real workflow runs rather than trusting it on paper

Out of scope:
- reading or printing the secret values themselves
- broad registry strategy beyond Quay and the existing GitHub registry flow

Decisions already made:
- Quay publish belongs inside the GitHub workflow-fix story as an additional task
- the repository already contains the required Quay secrets
- security must be treated very seriously for this work
- if a token or secret is actually read by the agent during implementation/debugging, an immediate report must be added under `.ralph/reports` so the user can rotate it
- only pushes to `main` may use these tokens
- no PR, no issue, and no random push to another branch may be able to use or read these secrets
- redaction and masking behavior must be explicitly verified
- Quay must be attempted first, before GitHub registry publication
- vulnerability failure in Quay must fail the workflow and block all later registry pushes

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers the workflow logic for Quay publication where practical
- [ ] The verify, SQL-emitter, and runner images are published to Quay only after required tests pass
- [ ] Quay image tags use the exact full commit SHA and do not rely on `latest` for the automated publish path
- [ ] Quay publication happens before GitHub Container Registry publication
- [ ] Quay vulnerability checking is a required gate, and any vulnerability failure fails the workflow and prevents later image pushes
- [ ] Quay publish secrets are usable only on pushes to `main` and are unavailable to pull requests, issues, other branches, forks, and other unintended workflow triggers
- [ ] Workflow steps avoid passing secrets on command lines where possible, use environment or STDIN-style mechanisms where supported, and explicitly mask any sensitive non-secret values with `::add-mask::`
- [ ] Real workflow-log verification proves redaction works and that Quay credentials do not leak in logs
- [ ] If a secret value is read during implementation/debugging, a rotation-warning report is added immediately under `.ralph/reports`
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-21-github-workflows-image-publish/06-task-publish-the-three-image-split-to-quay-with-strict-secret-redaction-and-main-only-access_plans/2026-04-20-explicit-quay-namespace-boundary-plan.md</plan>

## PO Override 2026-04-20

- Stop repeated local verification of the plan, Quay namespace boundary, or any supposed Quay secret source on this computer.
- Do not try to discover or inspect Quay secrets or secret values locally. Those GitHub-managed secrets are not meant to be visible here.
- Only check the outcome of the hosted GitHub workflow online.
- Instead of re-verifying the plan, start reading the hosted GitHub workflow logs/results online and use that observation as the source of truth.
- This task should observe whether the workflow actually works in GitHub rather than trying to prove it from local files or local environment inspection.
- If the hosted GitHub workflow is observed working online, set `<passes>true</passes>`.

## PO Extra Message 2026-04-20

- The only Quay robot secret names to use for this task are `QUAY_ROBOT_USERNAME` and `QUAY_ROBOT_PASSWORD`.
- Do not invent, search for, or depend on any other Quay secret names.
- Treat the repo/namespace pair supplied by the PO as `determined_keldysh / cockroach_migrate_tool`.
- Do not spend time guessing "namespace or whatever" from local state; use hosted workflow observation as the truth source and work from the PO-provided boundary above unless the hosted GitHub workflow itself proves otherwise.

## PO Follow-Up 2026-04-20

- Reread the task file and attached plan files before resuming. The currently recorded Quay namespace in those files is `determined_keldysh`.
- Change the Quay publish namespace first, before rerunning the hosted workflow that pushes to Quay.
- Prefer expressing that publish namespace through a GitHub variable if the workflow has access to one.
- If a GitHub variable cannot be used from the available workflow context, fill the namespace in directly and leave comments stating that the intended end state is an explicit GitHub variable-backed publish namespace.
