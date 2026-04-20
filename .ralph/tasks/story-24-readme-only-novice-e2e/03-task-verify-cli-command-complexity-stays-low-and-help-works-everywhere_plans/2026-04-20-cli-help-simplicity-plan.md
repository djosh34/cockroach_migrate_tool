# Plan: Verify CLI Command Complexity Stays Low And Help Works Everywhere

## References

- Task:
  - `.ralph/tasks/story-24-readme-only-novice-e2e/03-task-verify-cli-command-complexity-stays-low-and-help-works-everywhere.md`
- Current operator-facing contract:
  - `README.md`
- Current CLI definitions:
  - `crates/setup-sql/src/lib.rs`
  - `crates/runner/src/lib.rs`
  - `cockroachdb_molt/molt/cmd/root.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
- Existing CLI/help contracts:
  - `crates/setup-sql/tests/cli_contract.rs`
  - `crates/runner/tests/cli_contract.rs`
  - `crates/runner/tests/verify_image_contract.rs`
  - `crates/runner/tests/support/runner_docker_contract.rs`
  - `crates/runner/tests/support/runner_public_contract.rs`
  - `crates/runner/tests/support/verify_docker_contract.rs`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient approval for the public interface direction and the highest-priority behaviors to verify in this turn.
- This turn is planning-only because task 03 does not yet have a plan artifact or task-file `<plan>` pointer.
- The supported novice/operator surfaces are the command paths actually used by the README-driven flow:
  - `setup-sql emit-cockroach-sql --config ...`
  - `setup-sql emit-postgres-grants --config ...`
  - `runner validate-config --config ...`
  - `runner run --config ...`
  - verify image direct entrypoint help via `docker run <verify-image> --help`
  - verify-service runtime help via the underlying `molt verify-service run --help` contract
- Minimal command depth is measured from the user-visible entrypoint, not from every internal root:
  - `setup-sql` and `runner` should stay at one action level
  - the verify image is justified in keeping the internal `molt verify-service run` tree only because the published image entrypoint already lands the user on the verify-service runtime surface
- If execution finds a real command-surface defect or misleading help text, that is a product bug:
  - create a bug immediately via `add-bug`
  - ask for a task switch
  - keep `<passes>false</passes>`

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - the README-used command surfaces stay flat enough for novice/operator use
  - `--help` succeeds on every supported command or subcommand the operator may invoke
  - help output exposes the flags and descriptions the README relies on, especially `--config` and `--log-format`
  - help output rejects removed or unrelated surfaces that would make the novice path noisy or confusing
  - deeper nesting remains explicitly justified instead of creeping in accidentally
- Lower-priority concerns:
  - exact help formatting or whitespace
  - internal command trees that are not part of the README/operator path

## Current State Summary

- The current operator CLI contract is fragmented:
  - `crates/setup-sql/tests/cli_contract.rs` only checks root help text for the two subcommands
  - `crates/runner/tests/cli_contract.rs` checks runner root help, but not each README-used subcommand help surface directly
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go` checks `verify-service validate-config --help` and `verify-service run --help`
  - `crates/runner/tests/verify_image_contract.rs` checks published-image `--help`, but separately from the Go command tests
- The current ownership is muddy:
  - allowed subcommands live in `RunnerDockerContract`
  - removed-surface markers live in `RunnerPublicContract`
  - verify-image help markers live in `VerifyDockerContract`
  - setup-sql expectations are hardcoded directly in its test file
  - README-facing operator command expectations are therefore spread across multiple modules and languages
- The likely implementation work is contract consolidation plus a few missing help assertions, not a major CLI redesign:
  - `setup-sql` already has one action level
  - `runner` already has one action level
  - verify already hides its deeper internal root behind a direct image entrypoint

## Boundary Decision

- Add one dedicated operator CLI contract entrypoint that owns README-facing command simplicity and help expectations across all supported surfaces.
  - Preferred test entrypoint:
    - `crates/runner/tests/operator_cli_surface_contract.rs`
- Add one support owner for the typed operator command surface.
  - Preferred support file:
    - `crates/runner/tests/support/operator_cli_surface.rs`
- Keep existing per-image/per-binary tests for runtime, Dockerfile, and language-specific behavior, but stop letting them be the only place where the operator CLI contract is defined.
- During execution, prefer moving duplicated subcommand lists and forbidden-marker lists into the new operator CLI support owner instead of layering more ad hoc string checks onto the existing helpers.

## Intended Public Contract

- `setup-sql` must remain a flat two-command CLI for the README path:
  - `emit-cockroach-sql`
  - `emit-postgres-grants`
- `runner` must remain a flat two-command CLI for the README path:
  - `validate-config`
  - `run`
- The published verify image must keep the user-visible runtime surface direct:
  - `docker run <verify-image> --help` must explain the dedicated verify-service API and the required `--config` flag
  - unrelated `molt verify` command families must not leak into the published-image help
- The underlying verify-service command contract must keep README-relevant help discoverable:
  - `molt verify-service --help`
  - `molt verify-service validate-config --help`
  - `molt verify-service run --help`
- Help text must stay operator-usable:
  - mention the command purpose clearly
  - include required `--config` usage where relevant
  - include `--log-format` where the README documents it
  - exclude removed or unrelated flags such as source/target URL overrides, old verify helpers, or setup/runner-only commands on the wrong surface

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - README-facing command-shape rules are scattered across crate-specific tests, Docker/image helpers, and Go command tests with no single owner
- Required cleanup during execution:
  - make `operator_cli_surface.rs` the honest owner of:
    - allowed operator command families
    - maximum user-visible command depth
    - required help markers per surface
    - forbidden marker checks for removed or unrelated CLI surface
    - README-aligned command descriptions where that matters
  - shrink `RunnerDockerContract`, `RunnerPublicContract`, and `VerifyDockerContract` back toward image/runtime ownership instead of duplicate CLI-shape ownership
  - remove duplicated raw subcommand arrays or forbidden-marker lists once they are owned centrally
- Bold refactor allowance:
  - if existing helpers collapse naturally into a smaller number of support files after the new operator surface owner is introduced, delete the weaker ones instead of preserving them for symmetry

## Files And Structure To Add Or Change

- `crates/runner/tests/operator_cli_surface_contract.rs`
  - new dedicated task-03 contract for command simplicity and help behavior across setup-sql, runner, and verify
- `crates/runner/tests/support/operator_cli_surface.rs`
  - typed operator command metadata plus shared assertions for allowed depth, required help markers, and forbidden markers
- `crates/setup-sql/tests/cli_contract.rs`
  - reuse the shared operator CLI surface definitions where practical and add missing subcommand help assertions
- `crates/runner/tests/cli_contract.rs`
  - reuse the shared operator CLI surface definitions and add direct `validate-config --help` and `run --help` coverage if missing
- `crates/runner/tests/verify_image_contract.rs`
  - keep image-level coverage, but source shared verify help expectations from the operator CLI support owner
- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - extend or align the verify-service help tests so the Go command layer matches the shared operator contract
- `README.md`
  - adjust only if help text or command examples are shown to have drifted from the real CLI surface

## Vertical TDD Slices

### Slice 1: Tracer Bullet For One Honest Operator CLI Surface Contract

- RED:
  - add one failing test in `operator_cli_surface_contract.rs` that loads a shared operator command definition and proves the supported surfaces are explicitly enumerated in one place
  - fail first because the shared support owner does not exist yet
- GREEN:
  - add the smallest `operator_cli_surface.rs` support needed to represent the setup-sql, runner, and verify operator surfaces honestly
- REFACTOR:
  - move duplicated command-family literals out of existing tests/helpers into the new support owner

### Slice 2: Prove Setup-SQL Stays Flat And Help Works On Every README-Used Command

- RED:
  - add the next failing test that asserts:
    - `setup-sql --help` exposes only the two supported actions
    - `setup-sql emit-cockroach-sql --help` succeeds and includes `--config`
    - `setup-sql emit-postgres-grants --help` succeeds and includes `--config`
    - deeper nested actions are not exposed
- GREEN:
  - add the minimum shared assertions or CLI/help text fixes needed to satisfy that contract
- REFACTOR:
  - keep setup-sql help marker expectations data-driven in the shared support owner, not duplicated string checks

### Slice 3: Prove Runner Stays Flat And Help Works On Every README-Used Command

- RED:
  - add the next failing test that asserts:
    - `runner --help` exposes only `validate-config` and `run`
    - `runner validate-config --help` succeeds and includes `--config`
    - `runner run --help` succeeds and includes `--config`
    - removed source/verify surface markers stay absent
- GREEN:
  - add the minimum test-support or CLI/help adjustments needed to satisfy that contract
- REFACTOR:
  - collapse runner CLI expectations so command-family lists and forbidden markers are not split between multiple helpers

### Slice 4: Prove Verify Help Is Discoverable Both At The Published Image Surface And The Underlying Verify-Service Commands

- RED:
  - add the next failing test that asserts:
    - published verify-image `--help` succeeds and stays operator-focused
    - `verify-service --help` exposes only the README-relevant action layer
    - `verify-service validate-config --help` succeeds and includes `--config`
    - `verify-service run --help` succeeds and includes `--config`
    - unrelated `verify` CLI families do not leak into the image/operator path
- GREEN:
  - add the smallest verify-side help assertions or text changes needed to satisfy that contract
- REFACTOR:
  - keep verify image help markers and verify-service help markers defined from one shared surface spec instead of separate hardcoded lists

### Slice 5: Prove README Alignment And Justified Command Depth

- RED:
  - add a failing contract that encodes the README-facing command-shape policy:
    - setup-sql and runner stay at one action level
    - verify image stays direct for the user-visible surface
    - any deeper tree must be explicitly justified by the image entrypoint contract rather than silently allowed
  - assert the README-required flags and command descriptions remain visible in help output
- GREEN:
  - make the minimum shared-contract or help-text changes needed so the real CLI surface matches the README guidance
- REFACTOR:
  - represent depth/justification rules as typed data in the shared support owner instead of one-off assertions across multiple files

### Slice 6: Final Boundary Cleanup And Required Repository Lanes

- RED:
  - after the behavior slices are green, run:
    - `make check`
    - `make lint`
    - `make test`
  - do not run `make test-long` unless execution changes long-lane selection or the task explicitly proves it is required
- GREEN:
  - continue until every required default lane passes cleanly
- REFACTOR:
  - do one final `improve-code-boundaries` pass so operator CLI contract ownership is centralized and the remaining helpers each have one clear responsibility
- Stop condition:
  - if a real CLI/help defect is exposed during verification, create a bug immediately, ask for a task switch, and do not mark the task passed

## TDD Guardrails For Execution

- Every new behavior test must fail before the supporting code or help-text change is added.
- Do not satisfy this task with root-help checks alone. The README-used subcommands must be covered directly.
- Do not satisfy this task by scattering more string assertions across unrelated helpers. One operator CLI surface owner must define the contract.
- Do not allow verify’s deeper internal tree to justify extra depth on setup-sql or runner.
- Do not weaken the contract by removing operator-visible help details that the README depends on.
- Do not swallow command or help failures.
- Do not run `make test-long` unless the task explicitly requires it or execution changes ignored-test selection.

## Final Verification For The Execution Turn

- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] Do not run `make test-long` unless the task explicitly requires it or long-lane selection changes
- [ ] Final `improve-code-boundaries` pass confirms the operator CLI surface has one honest owner
- [ ] Update the task file checkboxes and set `<passes>true</passes>` only if no bug task was required

Plan path: `.ralph/tasks/story-24-readme-only-novice-e2e/03-task-verify-cli-command-complexity-stays-low-and-help-works-everywhere_plans/2026-04-20-cli-help-simplicity-plan.md`

NOW EXECUTE
