# Plan: Remove Runner Source Cockroach Access And Config

## References

- Task: `.ralph/tasks/story-16-runtime-split-removals/01-task-remove-runner-source-cockroach-access-and-config.md`
- Related prior plans:
  - `.ralph/tasks/story-09-verification-cutover/01-task-wrap-molt-verify-and-fail-on-log-detected-mismatches_plans/2026-04-19-molt-verify-wrapper-plan.md`
  - `.ralph/tasks/story-09-verification-cutover/02-task-build-drain-to-zero-and-cutover-readiness-check_plans/2026-04-19-drain-to-zero-cutover-readiness-plan.md`
- Current operator docs:
  - `README.md`
- Current implementation:
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/config/mod.rs`
  - `crates/runner/src/config/parser.rs`
  - `crates/runner/src/error.rs`
  - `crates/runner/src/helper_plan.rs`
  - `crates/runner/src/postgres_bootstrap.rs`
  - `crates/runner/src/runtime_plan.rs`
  - `crates/runner/src/schema_compare/mod.rs`
  - `crates/runner/src/molt_verify/mod.rs`
  - `crates/runner/src/cutover_readiness/mod.rs`
- Current tests and fixtures:
  - `crates/runner/tests/cli_contract.rs`
  - `crates/runner/tests/config_contract.rs`
  - `crates/runner/tests/schema_compare_contract.rs`
  - `crates/runner/tests/helper_plan_contract.rs`
  - `crates/runner/tests/verify_contract.rs`
  - `crates/runner/tests/cutover_readiness_contract.rs`
  - `crates/runner/tests/readme_contract.rs`
  - `crates/runner/tests/long_lane.rs`
  - `crates/runner/tests/fixtures/valid-runner-config.yml`
  - `crates/runner/tests/fixtures/container-runner-config.yml`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The acceptance criteria require the runner binary and runner config contract to expose no CockroachDB/source connection settings. In practice that means any public CLI surface that accepts `--source-url` or `--cockroach-schema` is in scope for removal now, not later.
- The runtime still needs source identity metadata that arrives through the webhook contract:
  - `mappings.source.database`
  - `mappings.source.tables`
  These are mapping facts, not source connection settings, and they remain necessary for routing, helper-table naming, and destination tracking.
- The runner should keep only destination-side responsibilities:
  - validate destination runtime config
  - render PostgreSQL grant/setup artifacts
  - bootstrap helper tables from destination catalog state
  - serve webhook ingest
  - run reconcile loops against PostgreSQL
- Source-side schema comparison, verification, and cutover verification no longer belong in `runner`, even if they were implemented there earlier.
- `render-helper-plan` currently mixes two responsibilities:
  - public source-schema artifact rendering
  - internal helper-table planning that `run` still needs
  This must be split so destination runtime planning survives while the source-facing CLI contract disappears.
- Story-16 task 02 overlaps with this task historically, but the current task’s acceptance is stricter. If execution removes `verify` and `cutover-readiness` entirely here, that is acceptable; the follow-up task can be replanned around any leftover verify-only docs or harness pieces instead of preserving forbidden source hooks now.
- If the first RED slice proves the runtime still depends on Cockroach schema input from the public CLI in order to start successfully, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Reduce the public runner CLI to destination-only commands:
  - keep:
    - `validate-config --config <path>`
    - `render-postgres-setup --config <path> --output-dir <dir>`
    - `run --config <path>`
  - remove:
    - `compare-schema`
    - `render-helper-plan`
    - `verify`
    - `cutover-readiness`
- Reduce `runner.yml` to destination runtime concerns only:
  - keep `webhook`, `reconcile`, and `mappings`
  - delete the entire `verify:` section and all parser/validation/reporting code behind it
- Keep runtime helper planning, but move it behind an internal destination-only boundary.
  - `MappingHelperPlan` and helper-table planning logic should remain available to `postgres_bootstrap` and `runtime_plan`
  - artifact rendering that depended on source schema files should be removed
- Keep `lib.rs` shallow and destination-focused:
  - parse only surviving CLI commands
  - load only destination runtime config
  - dispatch to destination-only modules
  - do not retain dead enums, output variants, or error conversions for removed source-facing commands

## Improve-Code-Boundaries Focus

- Primary smell: wrong-place source logic inside the runner binary.
  - `schema_compare`, `molt_verify`, and `cutover_readiness` are source-facing concerns living in the destination runtime crate
  - the fix is deletion, not indirection
- Primary config smell: unreduced config.
  - `RunnerConfig.verify` is not part of steady-state destination runtime behavior
  - delete the entire type family instead of carrying an inert `verify` subtree
- Secondary mixed-responsibility smell:
  - `helper_plan.rs` currently combines an internal helper-table planning model with a public artifact-rendering command that depends on source schema files
  - after removing the public source-facing command, keep only the internal planning model in a destination-owned module and delete the artifact-rendering path

## Public Contract To Establish

- `runner --help` lists only destination-side subcommands.
- `runner validate-config` succeeds for config files that contain no `verify:` section and reports no `verify=...` summary field.
- Runner config parsing rejects legacy `verify:` config as invalid/unknown.
- The runner image and README no longer document or expose:
  - `verify`
  - `cutover-readiness`
  - `compare-schema`
  - `render-helper-plan`
  - `--source-url`
  - `--cockroach-schema`
- `runner run` still boots and serves the destination runtime using only PostgreSQL access and mapping metadata already present in `runner.yml`.

## Files And Structure To Add Or Change

- [x] `crates/runner/src/lib.rs`
  - remove source-facing subcommands, removed output variants, and removed display branches
- [x] `crates/runner/src/config/mod.rs`
  - delete `VerifyConfig`, `MoltVerifyConfig`, and `verify_label`
- [x] `crates/runner/src/config/parser.rs`
  - delete raw verify parsing and ensure legacy `verify:` is rejected by the remaining config schema
- [x] `crates/runner/src/error.rs`
  - remove source-facing error families and any conversions tied only to deleted commands
- [x] `crates/runner/src/schema_compare/`
  - delete the entire module tree
- [x] `crates/runner/src/molt_verify/`
  - delete the entire module tree
- [x] `crates/runner/src/cutover_readiness/`
  - delete the entire module tree
- [x] `crates/runner/src/helper_plan.rs`
  - split internal helper planning from deleted artifact rendering, or replace the file entirely with a smaller destination-owned planning module
- [x] `crates/runner/src/postgres_bootstrap.rs`
  - rewire imports to the surviving internal helper-planning boundary only
- [x] `crates/runner/src/runtime_plan.rs`
  - rewire imports to the surviving internal helper-planning boundary only
- [x] `crates/runner/tests/cli_contract.rs`
  - assert removed subcommands and removed source flags are gone from public help
- [x] `crates/runner/tests/config_contract.rs`
  - remove verify-shaped expectations and add explicit failure coverage for legacy `verify:` config
- [x] `crates/runner/tests/readme_contract.rs`
  - replace source-facing README expectations with destination-only contract checks
- [x] `crates/runner/tests/long_lane.rs`
  - stop asserting `verify=...` in validate output while keeping the destination image contract green
- [x] Delete tests that exist only for removed public contracts:
  - `crates/runner/tests/schema_compare_contract.rs`
  - `crates/runner/tests/helper_plan_contract.rs`
  - `crates/runner/tests/verify_contract.rs`
  - `crates/runner/tests/cutover_readiness_contract.rs`
- [x] Update fixtures and test support that still write `verify:` config or invoke removed commands
- [x] `README.md`
  - remove runner-source instructions and make the runner quick start destination-only

## TDD Execution Order

### Slice 1: Tracer Bullet For The Reduced CLI And Config Contract

- [x] RED: add failing contract coverage that `runner --help` no longer lists `compare-schema`, `render-helper-plan`, `verify`, or `cutover-readiness`, and that `validate-config` output no longer prints a `verify=` field
- [x] GREEN: remove the source-facing subcommands from `lib.rs` and remove verify summary output from `ValidatedConfig`
- [x] REFACTOR: collapse `CommandOutput` and CLI dispatch so only destination-owned outputs remain

### Slice 2: Remove Verify Config From The Public YAML Schema

- [x] RED: add one failing config contract test that a legacy config containing `verify:` is rejected loudly, and one success-path test/fixture that validates a minimal config without `verify:`
- [x] GREEN: delete `RunnerConfig.verify`, parser branches, and associated config label code
- [x] REFACTOR: remove now-dead verify accessors and dead config DTOs entirely instead of leaving placeholders

### Slice 3: Delete Source-Schema Commands While Preserving Runtime Bootstrap

- [x] RED: extend CLI and README contract coverage so `compare-schema`, `render-helper-plan`, and `--cockroach-schema` are forbidden from the public runner contract
- [x] GREEN: delete `schema_compare` plus the public `render-helper-plan` path, update README, and keep `run` green by preserving only the internal helper-planning model derived from destination catalog state
- [x] REFACTOR: split the current `helper_plan.rs` so destination bootstrap logic no longer lives beside deleted source-schema artifact rendering

### Slice 4: Delete Verification And Cutover Source Access

- [x] RED: extend public contract coverage so `verify`, `cutover-readiness`, and `--source-url` are gone from CLI help, README, fixtures, and long-lane validate output
- [x] GREEN: delete `molt_verify`, `cutover_readiness`, their error types, and all tests/support that still route verification through the runner binary
- [x] REFACTOR: remove any lingering imports, helper functions, or test harness APIs that still imply the runner can compare against the source database

### Slice 5: Enforce Destination-Only Runner Boundaries

- [x] RED: add one destination-only regression test suite pass that proves the surviving public contract still works:
  - `validate-config`
  - `render-postgres-setup`
  - `run`
- [x] GREEN: fix the first breakage only, keeping the runner runtime healthy without reintroducing source-facing arguments or config
- [x] REFACTOR: confirm the remaining runner crate modules line up with destination runtime ownership only

### Slice 6: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to confirm no source-facing logic or config survives in the runner crate

## TDD Guardrails For Execution

- Every removal slice must start with a failing public-contract test before code deletion.
- Prefer contract tests through the surviving public CLI over implementation-coupled file-content assertions.
- Do not keep a compatibility shim for removed subcommands or removed config fields. This repo is greenfield and the task explicitly forbids backwards compatibility.
- Do not leave empty error enums, placeholder config structs, or dead modules behind just to preserve old shapes. Delete aggressively.
- Do not break `runner run` while removing source-facing commands. If runtime bootstrap still needs an internal helper-plan model, keep that model but move it behind a destination-owned module boundary.
- If execution reveals that a future story-16 task becomes mostly redundant because the forbidden source path is already deleted here, finish this task correctly anyway and leave the later task for replanning rather than preserving bad boundaries.

## Boundary Review Checklist

- [x] No runner CLI subcommand accepts `--source-url`
- [x] No runner CLI subcommand accepts `--cockroach-schema`
- [x] No `runner.yml` verify config exists
- [x] No validation output prints `verify=...`
- [x] No source-schema parser module remains in the runner crate
- [x] No in-runner verification module remains in the runner crate
- [x] Internal helper planning is destination-owned and no longer mixed with deleted source artifact rendering

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

Plan path: `.ralph/tasks/story-16-runtime-split-removals/01-task-remove-runner-source-cockroach-access-and-config_plans/2026-04-19-runner-source-access-removal-plan.md`

NOW EXECUTE
