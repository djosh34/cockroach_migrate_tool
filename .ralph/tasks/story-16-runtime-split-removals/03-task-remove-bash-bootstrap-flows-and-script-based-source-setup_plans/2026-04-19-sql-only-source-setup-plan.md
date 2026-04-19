# Plan: Remove Bash Bootstrap Flows And Make Source Setup SQL-Only

## References

- Task: `.ralph/tasks/story-16-runtime-split-removals/03-task-remove-bash-bootstrap-flows-and-script-based-source-setup.md`
- Related prior plan:
  - `.ralph/tasks/story-16-runtime-split-removals/01-task-remove-runner-source-cockroach-access-and-config_plans/2026-04-19-runner-source-access-removal-plan.md`
- Current operator docs:
  - `README.md`
- Current bash/script contract surface:
  - `crates/source-bootstrap/src/lib.rs`
  - `crates/source-bootstrap/src/render.rs`
  - `crates/source-bootstrap/tests/cli_contract.rs`
  - `crates/source-bootstrap/tests/bootstrap_contract.rs`
- Current runner contract and harness surface:
  - `crates/runner/tests/readme_contract.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/multi_mapping_harness.rs`
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- This task is not the final bootstrap-image implementation. It still must remove the current bash/script operator contract now.
- Greenfield rules apply. No compatibility shim should preserve `render-bootstrap-script`, `bash cockroach-bootstrap.sh`, executable temp scripts, or any shell-first public wording.
- The surviving source-setup contract should be emitted SQL text only:
  - SQL statements and optional SQL comments are allowed
  - shell shebangs, env-var assignments, `bash`, `sh`, `tail`, `cut`, and similar shell orchestration are not
- Long-lane runner coverage still needs a way to bootstrap Cockroach changefeeds. The correct replacement boundary is direct SQL execution, not a generated script file.
- If execution shows the repo needs a different command name or module split to keep the contract coherent, switch this plan back to `TO BE VERIFIED` immediately and stop.

## Interface And Boundary Decisions

- Keep a source-side generator only if it emits SQL and nothing else.
  - Public CLI contract:
    - keep the binary name `source-bootstrap`
    - replace `render-bootstrap-script` with `render-bootstrap-sql`
    - output pure SQL text that an operator or future image can execute without wrapper shell glue
- Update the README source-side story so it no longer teaches:
  - redirecting output into `cockroach-bootstrap.sh`
  - running `bash cockroach-bootstrap.sh`
  - treating the repo as the shipped shell-script contract
- Keep runner runtime behavior unchanged, but remove script-based setup from its tests.
  - runner long-lane harnesses should request SQL text and apply it through a direct Cockroach invocation boundary
  - no harness should materialize an executable bootstrap script or spawn `bash`
- Push shell/process mechanics down into one small test helper if needed, but keep the product-facing contract as plain SQL text.

## Improve-Code-Boundaries Focus

- Primary smell: `crates/source-bootstrap/src/render.rs` mixes the real product artifact with shell bootstrap orchestration.
  - Flatten this into a SQL-rendering boundary only.
  - Delete shell quoting helpers and shell-specific string assembly if they are no longer needed.
- Primary test smell: runner integration harnesses own temp-script paths, chmod logic, and `bash` execution even though the actual setup intent is "apply source bootstrap SQL".
  - Replace those fields and methods with a smaller SQL-only helper boundary.
  - Prefer deleting entire script-specific members and helper functions rather than renaming them in place.
- Secondary doc smell: README still presents shell script execution as the operator path.
  - Rewrite that contract to emitted SQL only and remove script-shaped examples and wording entirely.

## Public Contract To Establish

- `source-bootstrap --help` lists `render-bootstrap-sql` and does not list `render-bootstrap-script`.
- `source-bootstrap render-bootstrap-sql --config ...` emits SQL-only output:
  - contains the required cluster setting and changefeed SQL
  - contains no shebang, no `set -euo pipefail`, no shell variable assignments, and no `bash`
- README no longer instructs operators to create or execute `cockroach-bootstrap.sh`.
- Runner README contract tests and long-lane support assert the supported source setup path is SQL-only.
- Runner long-lane tests still bootstrap source-side changefeeds successfully without any generated shell artifact.

## Files And Structure To Add Or Change

- [x] `README.md`
  - rewrite the source setup section around emitted SQL only
- [x] `crates/source-bootstrap/src/lib.rs`
  - rename the public subcommand and keep dispatch SQL-only
- [x] `crates/source-bootstrap/src/render.rs`
  - replace or rename this module so it renders SQL text instead of shell text
- [x] `crates/source-bootstrap/tests/cli_contract.rs`
  - assert the new subcommand and removal of the script command
- [x] `crates/source-bootstrap/tests/bootstrap_contract.rs`
  - replace shell-script assertions with SQL-only contract assertions and failure-oriented forbidden-marker checks
- [x] `crates/source-bootstrap/tests/support/readme_contract.rs`
  - update source quick-start parsing if section markers or wording change
- [x] `crates/source-bootstrap/tests/fixtures/readme-source-bootstrap-config.yml`
  - keep only if the README still exposes a copyable source config block; otherwise delete it
- [x] `crates/runner/tests/readme_contract.rs`
  - forbid shell-script operator wording and assert SQL-only source setup wording
- [x] `crates/runner/tests/support/e2e_harness.rs`
  - delete script-path state and `bash` execution methods; replace with SQL-application helpers
- [x] `crates/runner/tests/support/multi_mapping_harness.rs`
  - make the same SQL-only harness reduction
- [x] `crates/runner/Cargo.toml`
  - keep or adjust the `source-bootstrap` dev-dependency only if it still cleanly serves the SQL contract used by tests
- [x] Remove dead script-only fixtures, helpers, and comments anywhere they survive after the refactor

## TDD Execution Order

### Slice 1: Tracer Bullet For The New Source Bootstrap CLI Contract

- [x] RED: add failing `source-bootstrap` CLI/help tests that require `render-bootstrap-sql` and explicitly reject `render-bootstrap-script`
- [x] GREEN: rename the subcommand and make the CLI dispatch the new SQL-only renderer
- [x] REFACTOR: collapse any command/output types that still assume script generation

### Slice 2: Make The Emitted Artifact SQL-Only

- [x] RED: replace one bootstrap contract test with SQL-only expectations:
  - required SQL statements remain present
  - forbidden shell markers fail the contract
- [x] GREEN: rewrite the renderer to output SQL text and optional SQL comments only
- [x] REFACTOR: delete shell-specific helpers, env-var construction, and shell quoting paths that no longer belong

### Slice 3: Update The Public README Contract

- [x] RED: add failing README contract coverage that forbids `bash cockroach-bootstrap.sh`, `.sh` handoff, and shell-first language in the source quick start
- [x] GREEN: rewrite the README so the source setup path is emitted SQL only and the destination quick start stays direct and explicit
- [x] REFACTOR: remove or rename README fixtures/support code so tests describe the new contract, not old section names

### Slice 4: Replace Script-Based Runner Test Harnesses With SQL Application

- [x] RED: make one long-lane/support contract fail because the harness still writes or executes a bootstrap script
- [x] GREEN: replace script materialization and `bash` execution with a direct "render/apply bootstrap SQL" helper path in `e2e_harness.rs` and `multi_mapping_harness.rs`
- [x] REFACTOR: delete temp-script fields, chmod logic, script-path plumbing, and any now-dead helper methods entirely

### Slice 5: Delete Remaining Script-Shaped Surface

- [x] RED: add one repo-level forbidden-surface assertion where needed so lingering public references to `render-bootstrap-script`, `cockroach-bootstrap.sh`, or bootstrap shell scripts fail loudly
- [x] GREEN: remove any leftover docs, fixture names, panic messages, comments, or test helpers that still describe the old script contract
- [x] REFACTOR: ensure the remaining module and fixture names reflect SQL setup rather than script setup wherever that rename improves clarity

### Slice 6: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to confirm no public or harness boundary still depends on generated shell scripts

## TDD Guardrails For Execution

- Start each removal slice with a failing contract test before code changes.
- Keep tests on public behavior:
  - CLI help
  - emitted artifact text
  - README/operator contract
  - harness-visible setup behavior
- Do not preserve `render-bootstrap-script` as an alias. No backwards compatibility is allowed.
- Do not keep dead script fields, dead fixture names, or "temporary" shell helpers after the SQL-only path is working.
- If a helper is needed for long-lane setup, it should express "apply emitted SQL" directly rather than "render and run script".

## Boundary Review Checklist

- [x] No public README step tells the operator to run `bash`
- [x] No public contract produces a `.sh` bootstrap artifact
- [x] No `source-bootstrap` command name includes `script`
- [x] No runner test harness writes an executable bootstrap script
- [x] No runner test harness spawns `bash` for source setup
- [x] The remaining source-setup artifact is SQL text only

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

Plan path: `.ralph/tasks/story-16-runtime-split-removals/03-task-remove-bash-bootstrap-flows-and-script-based-source-setup_plans/2026-04-19-sql-only-source-setup-plan.md`

NOW EXECUTE
