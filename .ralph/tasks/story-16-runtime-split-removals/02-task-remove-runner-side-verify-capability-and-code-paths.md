## Task: Remove verify behavior from the runner and delete every in-runner verification path <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Remove all code, commands, config, and tests that let the runner perform verification work. The higher order goal is to enforce the new system boundary where verification is performed only by a dedicated verify image over HTTP and never by the runtime that applies webhook events.

In scope:
- delete runner-side verify commands, libraries, wrappers, flags, config, and docs
- remove any in-process or sidecar verification path from the runner test harness
- add enforcement tests so the runner cannot regress into performing verify work

Out of scope:
- implementing the new verify HTTP service

Decisions already made:
- the runner must not verify because it is not allowed to access the original source database
- all correctness verification must move behind the dedicated verify image contract
- removal must be aggressive because this project is greenfield

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers removal of all runner-side verification behavior
- [ ] The runner image exposes no verify command, no verify config, and no verify code path
- [ ] Tests fail if correctness verification is attempted anywhere except the dedicated verify image contract
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
