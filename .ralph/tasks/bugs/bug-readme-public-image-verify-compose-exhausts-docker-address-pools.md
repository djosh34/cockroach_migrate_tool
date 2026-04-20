## Bug: README public-image verify compose verification exhausts Docker address pools <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
Story 24 verification found a blocking defect before the README-only public-image flow could be completed.

While executing `.ralph/tasks/story-24-readme-only-novice-e2e/01-task-verify-readme-alone-enables-a-full-public-image-migration-with-zero-repo-access.md`, the baseline verification contract
`cargo test -p runner --test novice_registry_only_contract -- --nocapture`
failed in `copied_compose_contracts_work_from_a_repo_free_operator_workspace`.

The failing step is the verify-service compose startup launched from the copied public artifact surface in
`crates/runner/tests/support/novice_registry_only_harness.rs`.

Observed failure:

`docker compose up verify failed with status exit status: 1`

`failed to create network cockroach-migrate-novice-verify-..._default: Error response from daemon: all predefined address pools have been fully subnetted`

This means the current README/public-image verification path is not robust: the compose-based verify step depends on a pristine Docker daemon with enough unused default bridge subnets, and leaked temporary compose networks can block the verification before any product-facing assertions run.

Detection evidence:
- failing contract: `cargo test -p runner --test novice_registry_only_contract -- --nocapture`
- failing test: `copied_compose_contracts_work_from_a_repo_free_operator_workspace`
- failing runtime message: `Error response from daemon: all predefined address pools have been fully subnetted`

The fix should make the README-only verification harness own Docker network lifecycle honestly and deterministically so this story can resume and reach the real public-image product assertions.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [x] I created a Red unit and/or integration test that captures the bug
- [x] I made the test green by fixing
- [x] I manually verified the bug, and created a new Red test if not working still
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [ ] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/bugs/bug-readme-public-image-verify-compose-exhausts-docker-address-pools_plans/2026-04-20-verify-compose-address-pool-plan.md</plan>
