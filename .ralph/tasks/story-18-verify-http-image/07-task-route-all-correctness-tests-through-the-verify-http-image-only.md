## Task: Route all correctness verification through the verify HTTP image only and remove all alternate test-harness paths <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Make the dedicated verify image the only supported correctness-verification path for tests. The higher order goal is to stop cheating in the test harness by verifying through a code path that production users will never have.

In scope:
- update test harnesses so correctness checks call the verify image over HTTP
- remove alternate in-process, direct-binary, or hidden verification routes used only by tests
- add enforcement tests so future test code cannot bypass the verify image contract

Out of scope:
- the business logic of verify itself beyond what is needed to expose the image contract

Decisions already made:
- all tests must verify correctness through the verify image
- there must not be a separate extra path inside the test harness

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers correctness verification through the verify HTTP image contract only
- [ ] Test suites fail if they bypass the verify image or use a hidden alternate verification route
- [ ] The supported test path matches the supported production verify path
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
