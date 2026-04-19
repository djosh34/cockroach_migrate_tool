# Plan: Build The One-Time Setup Image As The Sole SQL-Emission Surface

## References

- Task: `.ralph/tasks/story-19-source-sql-emitter-image/01-task-build-a-one-time-sql-emitter-image-that-prints-required-sql-to-logs.md`
- Related prior plans:
  - `.ralph/tasks/story-16-runtime-split-removals/03-task-remove-bash-bootstrap-flows-and-script-based-source-setup_plans/2026-04-19-sql-only-source-setup-plan.md`
  - `.ralph/tasks/story-16-runtime-split-removals/04-task-remove-novice-user-dependence-on-repo-clone-and-local-tooling_plans/2026-04-19-published-images-only-novice-path-plan.md`
  - `.ralph/tasks/story-04-source-bootstrap/02-task-apply-postgresql-helper-bootstrap-automatically_plans/2026-04-18-postgresql-helper-bootstrap-plan.md`
- Current one-time setup surface:
  - `crates/source-bootstrap/src/lib.rs`
  - `crates/source-bootstrap/src/render.rs`
  - `crates/source-bootstrap/tests/bootstrap_contract.rs`
  - `crates/source-bootstrap/tests/image_contract.rs`
- Current mixed runner/setup surface:
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/postgres_setup.rs`
  - `crates/runner/tests/config_contract.rs`
  - `crates/runner/tests/cli_contract.rs`
  - `crates/runner/tests/readme_contract.rs`
- Current published-image/docs surface:
  - `README.md`
  - `.github/workflows/master-image.yml`
- Story follow-on task stubs:
  - `.ralph/tasks/story-19-source-sql-emitter-image/02-task-emit-the-required-cockroach-changefeed-sql-from-the-one-time-setup-image.md`
  - `.ralph/tasks/story-19-source-sql-emitter-image/03-task-emit-the-absolute-minimum-postgresql-role-grants-needed-by-the-runner.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown is treated as approval for the public interface and behavior priorities in this turn.
- Greenfield rules apply. No compatibility alias should preserve the old split where:
  - Cockroach SQL comes from `source-bootstrap`
  - PostgreSQL grant SQL comes from `runner render-postgres-setup`
- The dedicated one-time setup image must become the only public place that emits manual setup SQL.
- The runner remains destination runtime only:
  - validate config
  - connect to PostgreSQL at startup
  - serve ingest/reconcile work
  - no manual grant-rendering subcommand
- The two setup commands must stay separate forever. A combined "emit everything" mode would recreate the boundary confusion this task is supposed to remove.
- Each setup command should require only the config it actually needs. Reusing the current `runner.yml` for PostgreSQL grant emission is the wrong boundary because it drags source-side config into a PostgreSQL-only manual step.
- If the first RED slice proves that renaming the current `source-bootstrap` slice into a broader setup-sql slice is too wide for this task to complete honestly, this plan must remain `TO BE VERIFIED` and execution must stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - a dedicated setup image starts directly from its binary entrypoint
  - the image exposes exactly two explicit SQL-emission subcommands, one Cockroach-only and one PostgreSQL-only
  - each subcommand supports `text` and simple `json` output modes
  - plain-text mode prints SQL to stdout instead of writing bash artifacts or directories of helper files
  - runner help, README, and image contracts no longer describe `render-postgres-setup`
- Lower-priority concerns:
  - preserving the old `source-bootstrap` naming
  - preserving directory-shaped PostgreSQL artifacts instead of switching to stdout SQL emission

## Problem To Fix

- The current setup boundary is split across two wrong places:
  - `crates/source-bootstrap` owns Cockroach setup emission
  - `crates/runner/src/postgres_setup.rs` owns PostgreSQL grant emission
- `runner render-postgres-setup` currently writes README/grants artifact directories, which makes the runtime image responsible for a one-time manual setup concern.
- The current published-image contract teaches two different setup surfaces:
  - a separate `source-bootstrap` image for Cockroach SQL
  - the runtime `runner` image for PostgreSQL grants
- The current config boundaries are also mixed:
  - the source setup config is source-only and lives under `crates/source-bootstrap`
  - the PostgreSQL grant output still depends on the full runner config shape even though that manual step should not need source-side config

## Interface And Boundary Decisions

- Replace the current `source-bootstrap` product boundary with one dedicated setup-sql boundary.
  - Preferred ownership:
    - crate directory renamed from `crates/source-bootstrap` to a setup-focused name
    - binary renamed away from `source-bootstrap`
    - published image renamed away from `cockroach-migrate-source-bootstrap`
  - Reason:
    - once the image emits both Cockroach and PostgreSQL SQL, keeping a source-only name is wrong-place naming and future mud
- The dedicated setup binary exposes exactly two top-level subcommands:
  - `emit-cockroach-sql`
  - `emit-postgres-grants`
- Each subcommand owns one explicit format flag:
  - `--format text`
  - `--format json`
  - default should be `text`
- Plain-text mode is stdout-only.
  - It may contain human SQL comments.
  - It must not create shell scripts, executable files, or README/grants directory trees.
- JSON mode stays simple.
  - One SQL string per database.
  - No mixing of Cockroach and PostgreSQL payloads in the same response.
  - No shell-shaped fields, status wrappers, or fake workflow metadata.
- Config stays reduced and mode-specific.
  - `emit-cockroach-sql` loads a Cockroach/webhook/mapping config only.
  - `emit-postgres-grants` loads a PostgreSQL grant config only.
  - Execution should not keep one combined config type with optional source/destination halves if that reintroduces validation drift.
- The runner public surface shrinks.
  - Keep `validate-config`
  - Keep `run`
  - Remove `render-postgres-setup`
- The setup image is a one-time operator tool only.
  - It does not apply SQL.
  - It does not become a runtime service.
  - It does not absorb runner bootstrap logic that must still happen automatically inside `runner run`.

## Improve-Code-Boundaries Focus

- Primary smell: wrong-place manual setup SQL in `crates/runner/src/postgres_setup.rs`.
  - Move or rewrite that logic under the dedicated setup-sql slice.
  - Delete the runner-side subcommand instead of keeping a compatibility shim.
- Primary naming smell: `source-bootstrap` becomes false once it owns PostgreSQL grants too.
  - Prefer a real rename over teaching tests to bless a misleading legacy name.
- Primary config smell: PostgreSQL grant emission currently depends on runner-owned config and runner-owned CLI.
  - Reduce this to a dedicated validated grant config inside the setup slice.
- Secondary output-shape smell: PostgreSQL grants are currently rendered as a directory tree plus README.
  - Flatten this to the same honest "stdout SQL or simple JSON" contract used by the setup image overall.
- Secondary helper smell: if the existing PostgreSQL renderer has single-caller helpers or artifact wrapper types, inline/delete them instead of transplanting file-writing abstractions into the new slice.

## Public Contract To Establish

- A published setup image exists and starts directly from its binary entrypoint.
- `--help` for the setup binary lists exactly the two SQL-emission subcommands and the shared output-format concept.
- `emit-cockroach-sql`:
  - requires only Cockroach/webhook/mapping config
  - supports `text` and `json`
  - emits the required Cockroach setup SQL to stdout/logs
- `emit-postgres-grants`:
  - requires only PostgreSQL grant config
  - supports `text` and `json`
  - emits the minimum PostgreSQL grant SQL to stdout/logs
- The two commands never share one combined output mode.
- `runner --help` and README no longer document `render-postgres-setup`.
- README shows the setup image as the operator-facing one-time SQL emitter for both setup domains, while `runner` stays the runtime image only.

## Files And Structure To Add Or Change

- [x] `Cargo.toml`
  - update workspace membership if the setup slice is renamed
- [x] `Cargo.lock`
  - accept package rename fallout if the crate/binary name changes
- [x] `crates/source-bootstrap/` or renamed successor directory
  - broaden or rename the existing slice into the one-time setup image owner
- [x] `crates/source-bootstrap/src/lib.rs` or renamed successor
  - replace the old source-only CLI with the new two-subcommand setup CLI
- [x] `crates/source-bootstrap/src/config/` or renamed successor
  - split config parsing into Cockroach-only and PostgreSQL-only validated boundaries
- [x] `crates/source-bootstrap/src/render.rs` or renamed successor modules
  - split renderers by setup domain and output format
- [x] `crates/source-bootstrap/src/error.rs` or renamed successor
  - keep errors typed and explicit for both config and render failures
- [x] `crates/source-bootstrap/tests/cli_contract.rs`
  - require the new setup CLI surface and reject legacy command names
- [x] `crates/source-bootstrap/tests/bootstrap_contract.rs`
  - evolve into end-to-end setup SQL output tests for both subcommands and both formats
- [x] `crates/source-bootstrap/tests/image_contract.rs`
  - require the dedicated setup image entrypoint and containerized contract for both commands
- [x] add or rename fixtures under `crates/source-bootstrap/tests/fixtures/`
  - separate Cockroach-only and PostgreSQL-only config fixtures
- [x] `.github/workflows/master-image.yml`
  - rename and publish the setup image under its new boundary if the product/image name changes
- [x] `README.md`
  - replace source-bootstrap wording and runner grant-rendering steps with one setup-image contract plus runtime-only runner steps
- [x] `crates/runner/src/lib.rs`
  - remove `RenderPostgresSetup` from the runner CLI
- [x] `crates/runner/src/postgres_setup.rs`
  - delete after the grant SQL rendering logic has moved or been rewritten in the setup slice
- [x] `crates/runner/tests/cli_contract.rs`
  - forbid the removed runner setup-rendering surface
- [x] `crates/runner/tests/config_contract.rs`
  - move grant-emission contract coverage out of runner-owned tests
- [x] `crates/runner/tests/readme_contract.rs`
  - update Docker quick start expectations for setup-image plus runtime-image separation
- [x] `crates/runner/tests/support/runner_docker_contract.rs`
  - reduce documented runner subcommands to runtime-only surface
- [x] `crates/runner/tests/support/readme_published_image_contract.rs`
  - update the published-image public contract around the renamed setup image

## TDD Execution Order

### Slice 1: Tracer Bullet For The Dedicated Setup Image Contract

- [x] RED: add one failing setup-image contract test that requires a direct binary entrypoint and exactly two explicit subcommands for Cockroach-only and PostgreSQL-only SQL emission
- [x] GREEN: rename or broaden the existing setup slice enough for the image and CLI help contract to pass
- [x] REFACTOR: remove any leftover source-only naming or runner-owned image knowledge that no longer fits the new boundary

### Slice 2: Introduce The Shared Output-Format Contract

- [x] RED: add one failing CLI/behavior test for `--format text|json`, with `text` printing SQL to stdout and `json` returning a simple per-database SQL mapping
- [x] GREEN: add the minimum output-format enum and command dispatch needed to pass
- [x] REFACTOR: keep format handling in one small typed boundary instead of duplicating string switches across both commands

### Slice 3: Keep Cockroach Setup Honest Inside The New Boundary

- [x] RED: port one existing Cockroach contract test to the new command name and new format contract, still rejecting bash/script markers
- [x] GREEN: move the current Cockroach renderer behind `emit-cockroach-sql` with the new stdout/json behavior
- [x] REFACTOR: delete legacy command names, source-only wrapper types, and any shell-era leftovers that survive the move

### Slice 4: Move PostgreSQL Grant Emission Out Of Runner

- [x] RED: add one failing contract test that requires PostgreSQL grant SQL to be emitted by the setup image and fails if `runner render-postgres-setup` remains documented or callable
- [x] GREEN: move or rewrite the PostgreSQL grant renderer under the setup slice and remove the runner subcommand
- [x] REFACTOR: flatten the old artifact-directory rendering into stdout/json SQL emission and delete file-writing wrappers that no longer belong

### Slice 5: Reduce Config To One Command-Specific Validated Shape Per Mode

- [x] RED: add failing config contract tests proving the Cockroach command does not require PostgreSQL fields and the PostgreSQL command does not require Cockroach/webhook fields
- [x] GREEN: introduce separate validated config parsers and fixtures for each command
- [x] REFACTOR: remove any optional-field mega-config or cross-command validation leakage

### Slice 6: Rewrite The Public Image And README Contract

- [x] RED: add failing README and image-contract assertions that require one setup image for manual SQL emission and a runtime-only runner image
- [x] GREEN: rewrite the README examples and published-image contract to use the new setup image name and command surface
- [x] REFACTOR: centralize setup-image coordinates in contract helpers and CI env so naming does not drift

### Slice 7: Repository Lanes

- [x] RED: run `make check`, `make lint`, and `make test`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required default lane passes cleanly
- [x] REFACTOR: skip `make test-long` unless execution changes the ultra-long lane or the task later proves it is required; finish with one final `improve-code-boundaries` pass

## TDD Guardrails For Execution

- Start each slice with a failing public-behavior test before the implementation change.
- Do not keep `runner render-postgres-setup` as a compatibility alias. No backwards compatibility is allowed.
- Do not satisfy the PostgreSQL side of this task by preserving directory-shaped artifacts as the public contract.
- Do not introduce a combined "emit all SQL" command or a config file that forces operators to provide both source and destination facts for every invocation.
- Do not keep the `source-bootstrap` product name if the image is no longer source-only. Wrong naming is a real boundary bug here.
- Do not swallow config, docker, or SQL-rendering errors. Fail loudly with typed errors and concrete messages.

## Boundary Review Checklist

- [x] Manual SQL emission no longer lives in `runner`
- [x] The setup image owns both Cockroach and PostgreSQL one-time SQL emission
- [x] The two setup commands stay separate and never share combined output
- [x] Each command has a reduced config boundary with only the fields it needs
- [x] Plain text means stdout SQL, not generated scripts or file trees
- [x] Runner docs and CLI are runtime-only after the refactor
- [x] No misleading source-only naming survives if the slice emits PostgreSQL grants too

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long` only if execution changes the ultra-long lane or the task explicitly proves it is required
- [x] One final `improve-code-boundaries` pass after all required lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every required lane passes

Plan path: `.ralph/tasks/story-19-source-sql-emitter-image/01-task-build-a-one-time-sql-emitter-image-that-prints-required-sql-to-logs_plans/2026-04-19-one-time-setup-image-plan.md`

NOW EXECUTE
