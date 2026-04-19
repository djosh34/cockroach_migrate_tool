# Plan: Emit The Exact Cockroach Changefeed SQL From The Setup Image

## References

- Task: `.ralph/tasks/story-19-source-sql-emitter-image/02-task-emit-the-required-cockroach-changefeed-sql-from-the-one-time-setup-image.md`
- Related prior plans:
  - `.ralph/tasks/story-19-source-sql-emitter-image/01-task-build-a-one-time-sql-emitter-image-that-prints-required-sql-to-logs_plans/2026-04-19-one-time-setup-image-plan.md`
  - `.ralph/tasks/story-04-source-bootstrap/01-task-build-cockroach-bootstrap-command-and-script-output_plans/2026-04-18-cockroach-bootstrap-command-and-script-output-plan.md`
  - `.ralph/tasks/story-16-runtime-split-removals/03-task-remove-bash-bootstrap-flows-and-script-based-source-setup_plans/2026-04-19-sql-only-source-setup-plan.md`
- Investigation evidence:
  - `.ralph/reports/po-mail-2026-04-18/full-project-report.txt`
  - `investigations/cockroach-webhook-cdc/output/summary.json`
- Current Cockroach setup surface:
  - `crates/setup-sql/src/lib.rs`
  - `crates/setup-sql/src/config/mod.rs`
  - `crates/setup-sql/src/config/cockroach_parser.rs`
  - `crates/setup-sql/src/render/cockroach.rs`
  - `crates/setup-sql/tests/bootstrap_contract.rs`
  - `crates/setup-sql/tests/image_contract.rs`
- Current runner-support dependency edge:
  - `crates/runner/Cargo.toml`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/multi_mapping_harness.rs`
- Public docs:
  - `README.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown is treated as approval for the public `emit-cockroach-sql` surface and the behavior priorities for this turn.
- The one-time setup image remains SQL-emission only. It must not execute Cockroach-side commands itself.
- The supported Cockroach setup contract is the one already justified by the repo investigation:
  - enable `kv.rangefeed.enabled`
  - capture an explicit `cluster_logical_timestamp()` starting cursor
  - create one webhook changefeed per source database
  - include `cursor`, `initial_scan = 'yes'`, `envelope = 'enriched'`, `enriched_properties = 'source'`, and configured `resolved`
- JSON output must remain simple: one SQL string per source database. It must not mix PostgreSQL work into the payload.
- Plain-text output may include human SQL comments, but the product artifact must remain SQL only and must not regress to shell glue.
- If the first RED slice proves that the SQL-only artifact cannot honestly express the explicit cursor handoff without reintroducing shell orchestration or a muddier public payload, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Problem To Fix

- The current `emit-cockroach-sql` path is close but incomplete for the intended supported flow:
  - it emits `SET CLUSTER SETTING kv.rangefeed.enabled = true;`
  - it emits `SELECT cluster_logical_timestamp();`
  - it emits `CREATE CHANGEFEED ...`
  - but it does not carry the explicit `cursor = ...` contract that the investigation and earlier story established as the correct restart boundary
- The current text output therefore describes part of the source setup but not the exact changefeed creation shape the project intends to support.
- The current module boundaries are also muddy:
  - `crates/setup-sql/src/config/mod.rs` owns both Cockroach and PostgreSQL config/domain types in one mixed module
  - `crates/setup-sql/src/render/cockroach.rs` owns both per-database planning and final string rendering directly
  - runner support tests link the `setup-sql` crate library directly instead of treating setup SQL generation as an external public boundary

## Interface And Boundary Decisions

- Keep the public command name:
  - `setup-sql emit-cockroach-sql --config <path> [--format text|json]`
- Keep Cockroach config reduced and source-only:
  - one Cockroach URL
  - one webhook sink configuration
  - one or more mappings with source database plus selected tables
  - no PostgreSQL fields anywhere in the Cockroach config parser
- The emitted Cockroach contract must make the explicit cursor handoff visible.
  - text mode should include a human-readable two-step SQL contract:
    - capture the cursor
    - create each changefeed with that explicit cursor
  - json mode should stay machine-friendly but still carry the exact per-database SQL string for the explicit-cursor changefeed contract
- Keep one SQL string per source database in JSON output.
  - shared preconditions may appear in each database SQL string if that is the clearest honest contract
  - do not introduce a second top-level metadata object just to smuggle in non-SQL state
- Keep webhook sink customization inside the Cockroach config boundary.
  - certificate file paths should be resolved and encoded during config loading
  - render code should only receive validated sink inputs, not filesystem paths
- Do not broaden this task into executing the SQL or returning job ids.
  - job id capture belonged to the old script-execution flow
  - this task is about the emitted SQL artifact only
- Runner/runtime separation should be proved, not hand-waved.
  - `runner` must not grow any source-side SQL generation surface
  - if feasible within the existing test harness shape, reduce runner support to consume the public setup-sql boundary instead of linking internal setup-sql Rust APIs directly

## Improve-Code-Boundaries Focus

- Primary smell: mixed command-domain ownership in `crates/setup-sql/src/config/mod.rs`.
  - Split or reduce the config/domain ownership so the Cockroach command does not live in a module that also owns PostgreSQL-only shapes.
  - Prefer moving Cockroach-owned domain types behind a Cockroach-specific module instead of leaving one mixed `mod.rs` as the grab bag.
- Primary smell: Cockroach rendering mixes planning and final formatting in one file.
  - Introduce a smaller typed per-database setup plan that owns the explicit cursor contract, then render text/json from that typed plan.
  - Keep SQL string assembly behind one boundary instead of spreading `format!` logic across config accessors and tests.
- Secondary smell: runner support tests call `source_bootstrap::execute(...)` directly.
  - If this can be removed cleanly, switch those harnesses to the public setup-sql invocation boundary and drop the dev-dependency.
  - If cargo/workspace constraints make that impossible without nested cargo invocations or major slowdown, keep the runtime boundary fixed and avoid inventing a worse test boundary.

## Public Contract To Establish

- `setup-sql --help` still lists `emit-cockroach-sql` and `emit-postgres-grants`.
- `setup-sql emit-cockroach-sql`:
  - requires only Cockroach/webhook/mapping config
  - supports `text` and `json`
  - emits SQL that includes the required cluster-setting step and the explicit-cursor changefeed contract
  - emits one SQL string per source database in JSON output
- Text output:
  - stays SQL-only plus SQL comments
  - includes clear operator-facing comments for the cursor capture handoff
  - does not contain shell markers, temp file instructions, or bash-era leftovers
- JSON output:
  - stays a top-level object keyed by source database
  - each value is one SQL string for that database
  - does not mix PostgreSQL grants or non-SQL workflow wrappers into the payload
- Runner/runtime contract:
  - no `runner` CLI command reclaims source-side setup
  - no runtime code path needs Cockroach source access after setup

## Files And Structure To Add Or Change

- [x] `.ralph/tasks/story-19-source-sql-emitter-image/02-task-emit-the-required-cockroach-changefeed-sql-from-the-one-time-setup-image.md`
  - add the execution-plan pointer for this task
- [x] `crates/setup-sql/src/config/mod.rs`
  - reduce or split mixed Cockroach/PostgreSQL ownership
- [x] `crates/setup-sql/src/config/cockroach_parser.rs`
  - tighten the Cockroach-only config parser around sink URL and certificate-path handling
- [x] `crates/setup-sql/src/render/cockroach.rs`
  - add the explicit cursor contract and, if helpful, a typed per-database setup plan before final rendering
- [x] `crates/setup-sql/src/lib.rs`
  - keep command dispatch thin after the Cockroach boundary cleanup
- [x] `crates/setup-sql/tests/bootstrap_contract.rs`
  - add contract coverage for explicit cursor rendering, multi-database JSON shape, and SQL-only text comments
- [x] `crates/setup-sql/tests/image_contract.rs`
  - keep the published-image entrypoint contract aligned with the new Cockroach SQL output
- [x] `crates/setup-sql/tests/fixtures/readme-cockroach-setup-config.yml`
  - update only if the supported webhook certificate-path contract changes
- [x] `README.md`
  - align the setup-sql quick start with the explicit-cursor changefeed contract
- [x] `crates/runner/Cargo.toml`
  - remove the `setup-sql` dev-dependency only if the public-boundary replacement is clean
- [x] `crates/runner/tests/support/e2e_harness.rs`
  - reduce any direct library coupling to setup-sql if that cleanup is practical in this task
- [x] `crates/runner/tests/support/multi_mapping_harness.rs`
  - apply the same boundary cleanup if the first harness slice proves it is straightforward

## TDD Execution Order

### Slice 1: Tracer Bullet For The Explicit Cursor SQL Contract

- [x] RED: add one failing `emit-cockroach-sql` contract test that requires the rendered changefeed SQL to expose the explicit cursor handoff instead of only `SELECT cluster_logical_timestamp();`
- [x] GREEN: implement the smallest Cockroach rendering change needed so the output expresses the explicit-cursor contract in text mode
- [x] REFACTOR: move Cockroach-specific setup planning out of mixed formatting code so the cursor contract lives in one typed place

### Slice 2: Keep JSON Simple While Preserving The Exact Changefeed Shape

- [x] RED: extend the Cockroach JSON contract test so each source-database SQL string includes the explicit-cursor changefeed contract and still stays a single string value
- [x] GREEN: render the updated per-database SQL strings in JSON without adding extra wrapper metadata
- [x] REFACTOR: centralize the per-database SQL plan so text and JSON share the same typed source of truth

### Slice 3: Keep Cockroach Config Reduced And Honest

- [x] RED: add failing config/behavior tests that prove the Cockroach command still requires no PostgreSQL fields and resolves its webhook certificate-path inputs correctly
- [x] GREEN: keep certificate-path reading and encoding inside the Cockroach parser, not the render layer
- [x] REFACTOR: apply `improve-code-boundaries` smell 11 by reducing the mixed config module and deleting cross-command config leakage

### Slice 4: Prove The Runner Boundary Stays Clean

- [x] RED: add or tighten one contract assertion that fails if `runner` reclaims source-side setup generation or source-access responsibilities
- [x] GREEN: if runner support still links setup-sql internals unnecessarily, switch it to a cleaner public boundary and remove the dev-dependency
- [x] REFACTOR: delete any dead setup-sql coupling in runner support once the public-boundary path is working

### Slice 5: Docs And Image Contract

- [x] RED: add failing README or image-contract assertions that require the explicit-cursor Cockroach setup wording and forbid stale incomplete examples
- [x] GREEN: update README/image expectations to match the real emitted SQL contract
- [x] REFACTOR: keep README fixtures and support helpers aligned with the single supported Cockroach config and output shape

### Slice 6: Repository Lanes

- [x] RED: run `make check`, `make lint`, and `make test`, fixing the first failing lane at a time
- [x] GREEN: continue until every required default lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so the remaining Cockroach setup path is not split across the wrong modules

## TDD Guardrails For Execution

- Start with one failing behavior test before each product change. Do not batch-write speculative tests.
- Keep tests on public behavior:
  - CLI output
  - config-loading behavior
  - image/README contract
  - runner/runtime boundary assertions
- Do not reintroduce shell artifacts, executable scripts, or bash placeholders.
- Do not invent PostgreSQL fields in the Cockroach config to make tests easier.
- If the explicit cursor contract requires a materially different output interface than one SQL string per database, switch back to `TO BE VERIFIED` instead of smuggling in a second protocol.

## Boundary Review Checklist

- [x] Cockroach config and PostgreSQL config no longer live in one muddy command-domain grab bag
- [x] Cockroach SQL planning and final rendering are separated cleanly enough that text/json share one typed source of truth
- [x] No filesystem path handling leaks from the parser into the render layer
- [x] No runner runtime code regains source-side setup behavior
- [x] No bash-era shell markers reappear in the Cockroach output
- [x] No error path is swallowed or downgraded to vague strings

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] One final `improve-code-boundaries` pass after all required lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every required lane passes

Plan path: `.ralph/tasks/story-19-source-sql-emitter-image/02-task-emit-the-required-cockroach-changefeed-sql-from-the-one-time-setup-image_plans/2026-04-19-cockroach-changefeed-sql-plan.md`

NOW EXECUTE
