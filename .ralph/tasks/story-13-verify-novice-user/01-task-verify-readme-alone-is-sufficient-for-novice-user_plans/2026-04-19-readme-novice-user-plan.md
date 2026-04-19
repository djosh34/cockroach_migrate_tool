# Plan: Verify README-Only Novice User Sufficiency

## References

- Task: `.ralph/tasks/story-13-verify-novice-user/01-task-verify-readme-alone-is-sufficient-for-novice-user.md`
- Adjacent story-13 tasks:
  - `.ralph/tasks/story-13-verify-novice-user/02-task-verify-direct-docker-build-and-run-without-wrapper-scripts.md`
  - `.ralph/tasks/story-13-verify-novice-user/03-task-verify-copyable-config-example-and-quick-start-clarity.md`
- Existing README contract:
  - `crates/runner/tests/readme_contract.rs`
- Existing public CLI contracts:
  - `crates/runner/tests/cli_contract.rs`
  - `crates/source-bootstrap/tests/cli_contract.rs`
- Existing runner command/artifact contracts:
  - `crates/runner/tests/config_contract.rs`
  - `crates/runner/tests/helper_plan_contract.rs`
  - `crates/runner/tests/schema_compare_contract.rs`
- Existing source bootstrap contract:
  - `crates/source-bootstrap/tests/bootstrap_contract.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown is treated as approval for the public interface and test priorities in this turn.
- This task is about README-only path closure for a novice user:
  - the operator should not need to inspect `crates/`, `tests/`, `investigations/`, or arbitrary repo files
  - the operator should not need hidden wrapper scripts
  - the README must define every required public artifact or point where the operator must provide one explicitly
- This task must stay separate from adjacent story-13 work:
  - task 02 owns proving the documented Docker commands work directly without wrapper scripts
  - task 03 owns making the config example and quick start copyable, concise, and directly useful
- A fast contract-first approach is appropriate here. The core failure mode is documentation contract drift, not a product runtime bug.
- If the first RED slice shows that README-only sufficiency cannot be expressed as a stable public contract without inventing a large markdown parser or speculative documentation DSL, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Problem To Fix

- `crates/runner/tests/readme_contract.rs` currently verifies only the write-freeze cutover wording.
- That leaves the novice quick-start path effectively unguarded:
  - the README can reference required inputs without saying how the operator gets them
  - the README can quietly depend on repo lore or source inspection
  - the README can drift from the actual public commands and generated artifacts without one place proving the path is closed
- The likely gap already visible in the current README is the schema-compare step:
  - it references `/schema/crdb_schema.txt` and `/schema/pg_schema.sql`
  - the README does not currently own an explicit way for a novice to obtain or provide those files inside the README-only path

## Boundary And Interface Decisions

- Keep all product CLIs unchanged unless the first RED slice proves a public command/help gap.
- Add one dedicated README contract support boundary instead of growing more raw string searches inside `crates/runner/tests/readme_contract.rs`.
  - preferred file: `crates/runner/tests/support/readme_contract.rs`
- The support boundary should own:
  - loading the repository README
  - locating named sections and ordered steps
  - locating documented command snippets and documented artifact paths
  - asserting that a novice path is closed over README-owned information
- The test file should own behavior statements only:
  - README documents a novice-ready source bootstrap path
  - README documents a novice-ready destination quick start
  - README does not require repo internals or source inspection
- Do not solve this with a generic markdown framework. Keep the support typed around this repo's public contract.

## Public Contract To Establish

- One fast README contract must fail if the novice path requires code or repo-internal inspection.
- One fast README contract must fail if the documented quick start references required artifacts that are not introduced or explained within the README itself.
- One fast README contract must fail if the README quick start drifts away from the public command surface already proven by CLI and artifact contracts.
- The README should explicitly own the public operator path across:
  - source bootstrap config and `render-bootstrap-script`
  - destination config and `validate-config`
  - schema-compare prerequisites and command
  - manual PostgreSQL setup artifact generation and what the operator does next
  - optional helper-plan review artifact generation
  - runtime startup

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - README verification knowledge currently lives as ad hoc phrase checks directly inside `crates/runner/tests/readme_contract.rs`
- Required cleanup during execution:
  - move README loading, section lookup, ordered-step assertions, and documented-artifact checks into one support module
  - keep `readme_contract.rs` behavior-focused instead of turning it into a second markdown parser
  - avoid a pile of single-use helper functions with overlapping string-search logic
- If the support module becomes a thin wrapper around trivial one-off calls, flatten it again. The goal is one honest owner, not another fake abstraction layer.

## Files And Structure To Add Or Change

- [x] `README.md`
  - likely add or tighten novice-path wording so every required quick-start input is README-owned
  - likely fix the schema-export prerequisite gap
- [x] `crates/runner/tests/readme_contract.rs`
  - expand coverage from cutover wording into README-only novice-path verification
- [x] `crates/runner/tests/support/readme_contract.rs`
  - preferred new support boundary for repository README loading and typed quick-start assertions
- [x] `crates/runner/tests/cli_contract.rs`
  - only if a README-owned runner command needs an explicit CLI contract assertion
- [x] `crates/source-bootstrap/tests/cli_contract.rs`
  - only if a README-owned source-bootstrap command/help detail needs stronger public-surface coverage
- [x] No product runtime code changes are expected
  - if execution discovers an actual command/help mismatch, change only the real public contract

## TDD Approval And Behavior Priorities

- Highest-priority behaviors to prove:
  - a novice can follow the README quick-start path without inspecting source code or arbitrary repo files
  - every required quick-start artifact is either generated by a documented command or explicitly provided by README-owned instructions
  - the README quick start names only public commands that exist and match the documented operator path
- Lower-priority concerns:
  - exact prose polish
  - aggressive markdown normalization

## Vertical TDD Slices

### Slice 1: Tracer Bullet For A Closed README-Only Destination Path

- [x] RED: add one failing README contract that models the destination quick start as a closed path and fails when a referenced required artifact is not introduced inside the README
- [x] GREEN: make the smallest README or contract-support change that closes the first real gap
- [x] REFACTOR: move README section and artifact lookup into the dedicated support boundary instead of leaving ad hoc string offsets in the test file

### Slice 2: Prove The README Does Not Require Repo-Internal Inspection

- [x] RED: add a failing contract that rejects novice-path dependence on repo internals such as `crates/`, `tests/`, `investigations/`, or instructions to inspect code
- [x] GREEN: tighten README wording or path references so the operator path stays on public artifacts only
- [x] REFACTOR: centralize the internal-path rejection rules in the README support boundary

### Slice 3: Prove README Commands Match Public CLI Surface

- [x] RED: add the next failing contract for the first README-owned command or generated artifact that is not already protected by an honest public contract
- [x] GREEN: strengthen the real CLI or artifact contract only where drift is currently possible
- [x] REFACTOR: keep command-surface expectations typed and shared instead of duplicating literal command names across unrelated tests

### Slice 4: Prove Source Bootstrap To Destination Runtime Reads As One README-Owned Journey

- [x] RED: add a failing contract that requires the README to connect source bootstrap, destination validation, schema comparison, manual grant artifacts, and runtime startup in operator order without “go look elsewhere” gaps
- [x] GREEN: add only the minimum README wording needed to make the path explicit
- [x] REFACTOR: keep ordered-step assertions inside the README support boundary so the behavior test reads like an operator journey, not phrase hunting

### Slice 5: Full Repository Lanes And Final Boundary Review

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so README verification has one honest owner and does not regress into scattered helper logic

## Guardrails For Execution

- Every new assertion must fail before the supporting README or test change is added.
- Do not satisfy this task by weakening the README path to a smaller promise than the task requires.
- Do not satisfy this task with README grep checks alone if a public CLI or artifact contract is actually missing; strengthen the real public contract where needed.
- Do not absorb task 02 by trying to prove full Docker execution end to end here.
- Do not absorb task 03 by turning this into a full rewrite of config copyability or all prose minimalism concerns.
- Do not introduce wrapper scripts, hidden repo-local commands, or “see source/tests” escape hatches to make the novice path pass.
- Do not swallow documentation-contract failures. If a required artifact or operator step is missing, the test should fail loudly with a concrete message.

## Boundary Review Checklist

- [x] One support boundary owns README loading and quick-start path assertions
- [x] `readme_contract.rs` reads as behavior specification, not string-search plumbing
- [x] Required quick-start artifacts are checked as README-owned inputs or outputs
- [x] Repo-internal path and source-inspection escapes are rejected explicitly
- [x] No error path is swallowed or hidden behind vague assertion messages

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
