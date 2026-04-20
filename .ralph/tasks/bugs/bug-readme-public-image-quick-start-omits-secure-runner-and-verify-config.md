## Bug: README public-image quick start omits secure runner and verify config <status>done</status> <passes>true</passes> <priority>ultra high</priority>

<description>
Story 24 execution is blocked because the README cannot currently serve as the only operator document for the secure public-image migration flow.

While executing the first README-only red slice for `.ralph/tasks/story-24-readme-only-novice-e2e/01-task-verify-readme-alone-enables-a-full-public-image-migration-with-zero-repo-access.md`, the public surface proved incomplete:

- `README.md:285-342` tells the operator to place `config/verify-service.yml` next to `verify.compose.yml`, but the README never includes an inline `verify-service.yml` example anywhere. A novice user therefore cannot derive the verify-service config from the README alone.
- `README.md:295-336` shows a `verify.compose.yml` snippet that still omits the required `verify-client-ca` config and mount, even though the shipped artifact now requires and provides it at `artifacts/compose/verify.compose.yml:17-18` and `artifacts/compose/verify.compose.yml:40-41`.
- `README.md:137-159` documents `config/runner.yml` with webhook TLS only. It does not document destination PostgreSQL TLS or mTLS fields, so it cannot satisfy the task’s required secure PostgreSQL connection path from README-only inputs.
- The current novice support harness is compensating with hidden repo-only inputs instead of README-owned ones:
  - `crates/runner/tests/support/novice_registry_only_harness.rs:312-373` copies cert fixtures from the repository and writes a hand-authored `config/verify-service.yml`.

This means the README-only public-image migration path is not the real product interface yet. The story task must stay blocked until the README owns the full secure operator contract.
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
