## Task: Debug hosted GitHub workflow failures, parallelize image builds with the test lanes, and surface Quay security findings clearly <status>not_started</status> <passes>false</passes>

<blocked_by>.ralph/tasks/story-23-e2e-duplicate-cdc-and-schema-mismatch/01-task-audit-and-add-full-e2e-coverage-for-duplicate-cdc-recreation-and-schema-mismatch.md</blocked_by>

<description>
Must use tdd skill to complete


**Goal:** Inspect the real hosted GitHub Actions failures and Quay security behavior, then optimize the workflow so the slowest work overlaps honestly instead of serializing unnecessarily. The higher order goal is to make the hosted CI and image-publish pipeline diagnosable and trustworthy: image build work should run in parallel with `make test` and `make test-long` where that does not weaken the validation gate, the true reason for the current pipeline failure must be identified from real logs instead of guessed locally, and the Quay security stage must show actual vulnerability findings or a clear scanner/policy failure instead of an opaque `exit 1`.

In scope:
- inspect the current failing GitHub Actions runs, jobs, and logs using authenticated access to the hosted workflow data rather than relying on local guesses
- keep this work in a separate new story at the end of the backlog instead of folding it into the earlier story-21 tasks
- treat `.ralph/tasks/story-23-e2e-duplicate-cdc-and-schema-mismatch/01-task-audit-and-add-full-e2e-coverage-for-duplicate-cdc-recreation-and-schema-mismatch.md` as a hard completion blocker for this task, because the long-lane workflow diagnosis must happen after that e2e CDC/schema-mismatch coverage work is complete
- restructure the workflow so container image build work runs in parallel with `make test` and `make test-long` where technically possible
- preserve the existing rule that publish/promotion/security decisions must still remain gated on the required validation lanes even if raw build work starts earlier
- determine whether `make test-long` itself is failing, or whether some other job, dependency edge, artifact handoff, scan step, or workflow condition is the real reason the pipeline goes red
- fix the real hosted failure mode rather than papering over symptoms or weakening the gates
- inspect the Quay security step closely, including the current behavior where the image is published but the step only exposes `exit 1`
- change the workflow/reporting so Quay vulnerability findings are visible when present, and scanner errors, policy failures, and vulnerability results are distinguishable from one another
- make it obvious in logs, summaries, or artifacts whether the image publish succeeded before the security step failed
- if execution uncovers a defect that cannot be fully fixed inside this task, create a bug immediately with the `add-bug` skill instead of leaving the failure or swallowed error untracked

Out of scope:
- weakening or bypassing the existing `make check`, `make lint`, `make test`, or `make test-long` gates just to make the workflow go green
- local-only workflow simulation treated as sufficient evidence
- unrelated CI cleanup outside the hosted failure path, build/test parallelism, and Quay security visibility problem

Decisions already made:
- this must live in some other story, not inside the existing story-21 GitHub workflow image-publish story
- this task must be blocked by the e2e CDC/schema-mismatch story-23 task before it can be completed
- raw image build work may be parallelized with `make test` and `make test-long`, but publish/promotion/security gating must remain honest and must not allow unvalidated images to count as released
- the current pipeline failure must be diagnosed from real hosted GitHub logs
- the current Quay security behavior is not acceptable because `exit 1` without surfaced findings is not an operator-usable result
- if vulnerabilities exist, the workflow must show them clearly rather than hiding behind a generic failure code
- if the security step is failing for some other reason, that failure must be made explicit rather than confused with published-image success
- no error path may be swallowed or silently downgraded during this investigation

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers the workflow contract changes where practical, including the intended parallel build/validation topology and visible failure surfacing
- [ ] Real hosted GitHub Actions runs, jobs, and logs have been inspected through authenticated access, and the actual current failure mode or modes are recorded and fixed
- [ ] Container image build work runs in parallel with `make test` and `make test-long` where technically possible, without weakening the existing validation gate before publish/promotion decisions
- [ ] The task proves whether `make test-long` itself is failing or whether another pipeline stage is the real blocker, and fixes the real blocker rather than masking it
- [ ] The Quay security stage no longer fails as an opaque `exit 1`; workflow output clearly distinguishes scanner failure, policy failure, and vulnerabilities found
- [ ] If vulnerabilities are present in Quay, the workflow exposes those findings clearly enough for an operator to see what they are
- [ ] If the image is already published before the security step fails, the workflow makes that state explicit so publish success and scan failure are not conflated
- [ ] Any newly discovered defect that cannot be fixed inside this task is immediately captured as a bug via `add-bug`
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
