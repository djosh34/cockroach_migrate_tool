## Bug: Verify Compose novice-user contract omits the listener client CA mount <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
The documented registry-only novice-user flow for `verify.compose.yml` is broken.

Story-25 verification added a repo-free temp-workspace contract that copies the shipped `artifacts/compose/verify.compose.yml`, writes only operator-owned config files, sets `VERIFY_IMAGE`, and runs `docker compose up verify`.

That startup fails immediately with:

`open /config/certs/client-ca.crt: no such file or directory`

The generated `verify-service.yml` requires `listener.tls.client_auth.client_ca_path: /config/certs/client-ca.crt`, but the shipped `verify.compose.yml` does not mount any config entry at that path. The novice-user contract therefore cannot complete from published images and copied Compose artifacts alone.

Detection evidence:
- failing contract: `cargo test -p runner --test novice_registry_only_contract copied_compose_contracts_work_from_a_repo_free_operator_workspace -- --exact`
- failing runtime log:
  - `{"level":"error","service":"verify","event":"command.failed","message":"open /config/certs/client-ca.crt: no such file or directory"}`
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

<plan>.ralph/tasks/bugs/verify-compose-missing-client-ca-config-mount_plans/2026-04-20-verify-compose-client-ca-mount-plan.md</plan>
