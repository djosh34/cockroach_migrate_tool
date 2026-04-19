# Plan: Code-Complexity And KISS Assessment Report

## Scope

Produce the required report artifacts for `.ralph/tasks/story-14-reports/02-task-report-code-complexity-and-kiss-assessment.md` by inspecting the codebase exactly as it exists on disk. This is an analysis-and-artifacts task, not a speculative design essay. Every conclusion must be grounded in source files, tests, manifests, or README-visible contracts that are present during execution.

## Required Artifacts

- `.ralph/tasks/story-14-reports/artifacts/report-code-complexity/summary.md`
- `.ralph/tasks/story-14-reports/artifacts/report-code-complexity/module-inventory.md`
- `.ralph/tasks/story-14-reports/artifacts/report-code-complexity/interaction-analysis.md`
- `.ralph/tasks/story-14-reports/artifacts/report-code-complexity/complexity-findings.md`
- `.ralph/tasks/story-14-reports/artifacts/report-code-complexity/kiss-recommendations.md`

## Public Interface Under Test

This task does not test one user-facing CLI behavior. The public contract under test is the codebase structure itself as exposed through:

- workspace manifests: `Cargo.toml`, crate manifests, and dependency boundaries
- crate entrypoints: `crates/runner/src/lib.rs`, `crates/runner/src/main.rs`, `crates/source-bootstrap/src/lib.rs`, `crates/source-bootstrap/src/main.rs`
- crate module trees under `crates/runner/src/` and `crates/source-bootstrap/src/`
- contract tests under `crates/runner/tests/` and `crates/source-bootstrap/tests/`
- the operator-facing README where it reflects the intended runtime surface

The report must distinguish between:

- public CLI/runtime contracts
- internal planning and execution modules
- test-only harness structure
- implementation details that should not be mistaken for stable public surface

## TDD Execution Strategy

Use vertical slices even though the output is documentation. Do not read every file first and then dump a report. For each slice:

1. `RED`: define the next specific behavioral question the report must answer.
2. Read only the source files needed to answer that question from public boundaries inward.
3. `GREEN`: write or update the corresponding artifact section immediately with concrete evidence.
4. `REFACTOR`: tighten wording, remove duplicated observations, and move conclusions to the artifact where they actually belong.

The artifacts are the executable output of this task. Each slice must leave the report in a more complete, evidence-backed state.

## Behavior Slices To Execute

### Slice 1: Workspace inventory and dominant seams

- Read the top-level `Cargo.toml`, crate manifests, README workspace layout, and crate entrypoints.
- Confirm what crates exist, which one owns most behavior, and which boundaries are intentionally thin versus operationally deep.
- Start `summary.md` and `module-inventory.md` immediately with:
  - workspace size and composition
  - each crate's apparent responsibility
  - first-pass assessment of whether the workspace shape feels simple or already fragmented

### Slice 2: Module inventory from real source roots

- Read every module root under `crates/runner/src/` and `crates/source-bootstrap/src/`.
- Inventory every module and submodule that actually exists on disk.
- Record for each module:
  - primary responsibility
  - major types/functions exported or consumed
  - whether the module is a boundary, orchestration layer, data-shape layer, or utility
  - whether its responsibility feels clean or blurred
- Write those findings directly into `module-inventory.md`.

### Slice 3: Interaction flow and dependency map

- Starting from the crate entrypoints, trace how data and control move through:
  - config loading
  - startup planning
  - bootstrap/setup helpers
  - webhook ingest
  - reconcile execution
  - schema compare
  - verify/cutover-readiness
- Capture which modules call which other modules and whether those interactions are direct, layered, or circuitous.
- Write `interaction-analysis.md` from real call paths, not inferred architecture diagrams.

### Slice 4: Complexity hotspot analysis

- Read the deeper implementation modules where complexity is most likely to accumulate, prioritizing:
  - `runtime_plan.rs`
  - `helper_plan.rs`
  - `postgres_bootstrap.rs`
  - `tracking_state.rs`
  - `validated_schema.rs`
  - `webhook_runtime/{mod,payload,routing,persistence}.rs`
  - `reconcile_runtime/{mod,upsert,delete}.rs`
  - `schema_compare/{mod,cockroach_export,postgres_export,report}.rs`
  - `molt_verify/mod.rs`
  - `cutover_readiness/mod.rs`
- Identify where complexity is justified by real domain constraints and where it looks ornamental, duplicated, overly layered, or stringly.
- Record each finding in `complexity-findings.md` with:
  - exact file references
  - why it increases or reduces reasoning cost
  - whether the complexity appears necessary, accidental, or currently uncertain

### Slice 5: Test-surface cross-check

- Read contract tests and harness support only after the source-level understanding exists.
- Use tests to validate whether the implementation boundaries are reflected cleanly in behavior, or whether tests reveal hidden coupling, oversized harnesses, or architecture that is harder to exercise than it should be.
- Fold only structural insights into the report; do not let test helper internals dominate the analysis.

### Slice 6: KISS and boundary assessment

- Apply the `improve-code-boundaries` mindset explicitly after the inventory and hotspot passes.
- Look for one or more boundary problems such as:
  - duplicate shapes across config/startup/runtime layers
  - modules that mostly translate one shape into another without deepening the abstraction
  - stringly rendering or label-building that leaks across layers
  - bootstrap/runtime/reporting concerns mixed together
  - test harness complexity that mirrors production complexity instead of exposing a simpler seam
- Write `kiss-recommendations.md` with concrete flattening recommendations grounded in current files.
- Ensure the recommendations favor removing types/modules/conversions over adding more wrappers.

### Slice 7: Report synthesis and consistency pass

- Tighten `summary.md` so it answers plainly:
  - Is the codebase mostly KISS-oriented or drifting into complexity?
  - Which modules feel stable and easy to reason about?
  - Which boundaries are most likely to become muddy next?
- Remove duplicated observations across artifacts.
- Ensure every recommendation in `kiss-recommendations.md` has supporting evidence elsewhere in the report.

## Evidence Rules

- Every non-trivial claim must point back to files actually read during execution.
- Prefer file-path references over vague language such as "some runtime code" or "a config layer."
- Distinguish facts from interpretations. Facts come from code on disk; interpretations are the KISS/complexity judgments drawn from those facts.
- If a conclusion is uncertain because the code surface is ambiguous, say so explicitly rather than smoothing over it.

## Improve-Code-Boundaries Rule

This task is analysis-first, but it still must use the `improve-code-boundaries` skill as part of execution:

- perform one explicit smell pass focused on module boundaries and duplicate shapes
- prioritize recommendations that remove files, types, or translation layers when they are not buying clarity
- if execution reveals a tiny blocker fix is strictly necessary to produce accurate artifacts, prefer flattening over adding another abstraction
- before closing the task, confirm the report itself is not muddy: keep inventory, interaction mapping, findings, and recommendations in separate artifacts with minimal duplication

## Validation And Finish

After the report artifacts are complete and any necessary supporting fixes are done:

- run `make check`
- run `make lint`
- run `make test`
- run `make test-long`
- confirm the task file can be updated to `<passes>true</passes>` only after all four pass

## Expected Execution Notes

- This workspace is small enough that execution should inspect nearly all production source files, not just a sample.
- The `runner` crate appears to be the dominant complexity center; execution should treat `source-bootstrap` and `ingest-contract` as contrast points when assessing whether complexity is localized or leaking.
- If execution discovers that the report cannot make a clean claim because current module names or responsibilities are too ambiguous, switch this plan back to `TO BE VERIFIED` and stop immediately rather than forcing a low-trust report.

Plan path: `.ralph/tasks/story-14-reports/02-task-report-code-complexity-and-kiss-assessment_plans/2026-04-19-code-complexity-kiss-report-plan.md`

NOW EXECUTE
