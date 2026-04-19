## Task: Run a five-pass security audit of the verify HTTP surface and file bugs for every issue found <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Perform a deep security audit of the verify HTTP image and its input handling with five separate deliberate passes before the workstream is considered complete. The higher order goal is to treat the new remotely-triggered verify surface as hostile by default and force repeated review instead of one shallow pass.

In scope:
- five explicit audit passes against the HTTP API, job creation, config handling, process launching, result retrieval, metrics exposure, TLS material handling, and resource exhaustion risks
- documentation of findings per pass
- creating an `add-bug` task for every security issue found
- verification that no errors are swallowed or ignored during the audit work

Out of scope:
- implementing every bug fix inside this audit task

Decisions already made:
- POST input is fully untrusted and must be reviewed accordingly
- the audit task is not complete until it has been done five times
- every discovered security issue must become a separate bug task

</description>


<acceptance_criteria>
- [x] Red/green TDD covers security-sensitive input handling and explicit non-shell invocation guarantees where practical
- [x] Five distinct audit passes are recorded with concrete findings or explicit no-finding conclusions
- [x] Every discovered security issue results in a separate bug task created via `add-bug`
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-18-verify-http-image/09-task-run-a-five-pass-security-audit-of-the-verify-http-surface-and-file-bugs-for-each-issue_plans/2026-04-19-verify-http-five-pass-security-audit-plan.md</plan>
