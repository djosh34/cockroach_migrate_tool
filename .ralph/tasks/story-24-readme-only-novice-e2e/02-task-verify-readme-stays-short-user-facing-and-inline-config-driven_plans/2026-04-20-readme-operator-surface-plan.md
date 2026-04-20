# Plan: Verify README Operator Surface Stays Short, Inline, And User-Facing

## References

- Task:
  - `.ralph/tasks/story-24-readme-only-novice-e2e/02-task-verify-readme-stays-short-user-facing-and-inline-config-driven.md`
- Current operator-facing surface:
  - `README.md`
- Existing README/public-image contracts:
  - `crates/runner/tests/novice_registry_only_contract.rs`
  - `crates/runner/tests/support/readme_public_image_workspace.rs`
  - `crates/runner/tests/support/novice_registry_only_harness.rs`
- Existing public-image artifact contracts:
  - `artifacts/compose/setup-sql.compose.yml`
  - `artifacts/compose/runner.compose.yml`
  - `artifacts/compose/verify.compose.yml`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is treated as approval for the public interface direction and the highest-priority behaviors to test in this turn.
- This task is planning-only because there is no existing task-02 plan artifact yet.
- The README itself is the product boundary under test here, not contributor docs, internal fixtures, or implementation comments.
- The story-24 task-01 runtime contract already proved the README can drive a real public-image flow.
- Task 02 is narrower and stricter:
  - prove the README stays short
  - prove it talks only to the operator
  - prove config examples remain inline and copyable
  - prove contributor/process/philosophy content is absent from the README rather than merely separated by heading
- If execution shows that the current README still requires top-level contributor/process material for internal reasons, that is a product defect:
  - create a bug immediately via `add-bug`
  - ask for a task switch
  - keep `<passes>false</passes>`

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - the README starts the operator flow with a simple working setup example before adding extra arguments or extra deployment surfaces
  - the README keeps all required config examples inline as fenced blocks for the supported image flows
  - the README explains required and optional arguments in direct list form instead of diffuse prose
  - the README rejects contributor-only guidance, philosophy, and repo-structure discussion anywhere in the supported operator document
  - the README stays short enough to behave like a quick-start guide rather than a mixed operator-plus-maintainer manual
- Lower-priority concerns:
  - exact prose wording where the contract is already direct and operator-oriented
  - legal or contributor material that can safely move out of `README.md` into purpose-built files without weakening the public interface

## Current State Summary

- The current README is visibly wider than the task allows:
  - it starts with repository framing before the operator guide
  - it contains a top-level contributor-workflow pointer
  - it contains a `## Licensing` section
  - it contains a `## CI Publish Safety` section
  - it is currently about 1392 words with six second-level sections
- The current test ownership is muddy:
  - `novice_registry_only_contract.rs` mixes runtime-image behavior, artifact YAML assertions, and README phrase checks in one file
  - `readme_public_image_workspace.rs` owns README block extraction, repo-free checks, and operator workspace materialization at the same time
  - task-02 concerns about brevity, ordered operator sections, and contributor/process absence do not yet have one honest owner
- The README already contains most inline config and Compose text needed for the supported flows, so the likely execution work is contract tightening plus README pruning/reordering rather than inventing a new operator path.

## Boundary Decision

- Split README-content verification away from the heavy Docker-runtime contract.
  - Preferred new test entrypoint:
    - `crates/runner/tests/readme_operator_surface_contract.rs`
- Introduce one support owner for README structure and content rules.
  - Preferred support file:
    - `crates/runner/tests/support/readme_operator_surface.rs`
- Keep `readme_public_image_workspace.rs` focused on extracting/materializing the operator workspace only.
- Remove duplicate README string scans from `novice_registry_only_contract.rs` once the new content contract owns them.
- During execution, prefer deleting now-duplicated phrase-check code instead of layering more helpers onto the current mixed contract.

## Intended Public Contract

- The top-level README is the operator guide and should keep only the supported image flows plus the minimum context needed to run them.
- The README must begin the operator path with the simplest supported setup example:
  - setup-sql image first
  - then PostgreSQL grants
  - then runner
  - then verify
- The README must keep these inline fenced artifacts as the honest public examples:
  - `config/cockroach-setup.yml`
  - `config/postgres-grants.yml`
  - `config/runner.yml`
  - `config/verify-service.yml`
  - `setup-sql.compose.yml`
  - `runner.compose.yml`
  - `verify.compose.yml`
- The README must explain required and optional arguments in simple list form near the commands that use them, rather than through long explanatory paragraphs.
- The README must fail the contract if it includes operator-irrelevant material such as:
  - contributor workflow pointers
  - repo-structure guidance
  - process sections like CI publish policy
  - project philosophy or “why this matters” framing
  - repository build/test instructions in the supported user path

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - README content-contract knowledge is split across runtime tests, README workspace extraction, and ad hoc string checks
- Required cleanup during execution:
  - make `readme_operator_surface.rs` the honest owner of:
    - section extraction
    - heading order
    - forbidden-content checks
    - inline fenced-block lookup
    - quick-start brevity/shape assertions
  - shrink `readme_public_image_workspace.rs` back to workspace extraction/materialization only
  - remove README phrase checks from `novice_registry_only_contract.rs` once covered by the dedicated content contract
- Bold refactor allowance:
  - if `readme_public_image_workspace.rs` and the new README surface helper naturally collapse into one smaller, cleaner owner without mixed responsibilities, merge them and delete the weaker file

## Files And Structure To Add Or Change

- `README.md`
  - remove or relocate non-operator sections from the top-level README
  - reorder the operator path so the simplest example comes first and optional surfaces follow
  - convert argument explanation into short required/optional lists where the contract currently relies on prose
- `crates/runner/tests/readme_operator_surface_contract.rs`
  - new dedicated task-02 content contract for shortness, operator focus, inline config, and forbidden README content
- `crates/runner/tests/support/readme_operator_surface.rs`
  - structured README support for section order, code-block extraction, and brevity/content assertions
- `crates/runner/tests/novice_registry_only_contract.rs`
  - remove now-duplicated README-content checks so it returns to runtime/artifact ownership
- `crates/runner/tests/support/readme_public_image_workspace.rs`
  - narrow it to workspace materialization or merge it into the new owner if that is cleaner

## Vertical TDD Slices

### Slice 1: Tracer Bullet For A Dedicated README Operator-Surface Contract

- RED:
  - add one failing test in `readme_operator_surface_contract.rs` that loads the README through a dedicated support boundary and asserts the operator document is isolated from contributor/process content
  - fail on the currently visible top-level noise such as contributor-workflow guidance and CI publish policy
- GREEN:
  - add the smallest structured README support needed to load sections and assert forbidden content honestly
- REFACTOR:
  - move duplicated README slicing logic out of `novice_registry_only_contract.rs` and into the dedicated support owner

### Slice 2: Prove The README Starts Simple And Grows Only When Needed

- RED:
  - add the next failing test that asserts the README starts with the simplest supported setup example and only introduces runner/verify-specific extras after that base flow
  - reject a reference-manual ordering where optional or advanced material appears before the basic path
- GREEN:
  - make the smallest README reordering/edit needed to satisfy the operator-first sequence
- REFACTOR:
  - keep ordered-heading and ordered-code-block assertions inside the support boundary instead of scattering raw index math in tests

### Slice 3: Prove Required And Optional Arguments Stay In Short Operator Lists

- RED:
  - add a failing test that requires each supported image surface to explain required and optional args in short list form near the relevant commands
  - reject long narrative prose as the only explanation path
- GREEN:
  - tighten the README into short required/optional lists without expanding it into a reference manual
- REFACTOR:
  - represent per-section argument expectations as small typed fixtures or constants owned by the README support boundary rather than repeating raw strings in multiple tests

### Slice 4: Prove Inline Config And Compose Examples Remain The Honest Public Interface

- RED:
  - add the next failing test that asserts the exact inline config files and Compose files still exist in fenced blocks in the README and are positioned inside the operator flow
  - reject any drift toward “see repo file X” or “copy this from elsewhere”
- GREEN:
  - make the minimum README/support changes needed so inline examples remain complete and copyable
- REFACTOR:
  - collapse duplicate fenced-block extraction between the new content helper and the existing README workspace helper so there is one honest parser path

### Slice 5: Prove README Brevity Without Regressing The Supported Flow

- RED:
  - add a failing test that encodes a concrete quick-start shape budget for the operator surface
  - likely budget dimensions:
    - allowed top-level sections
    - maximum heading count inside the operator path
    - maximum prose-only paragraphs between commands/examples
  - avoid a fragile exact-word-count gate unless execution proves it is the only honest way to catch regressions
- GREEN:
  - remove or relocate README noise until the operator guide is short and direct again
- REFACTOR:
  - keep the brevity policy data-driven in the README support owner, not as one-off literals across tests

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
  - do one final `improve-code-boundaries` pass so README structure/content ownership is separated cleanly from runtime-workspace ownership
- Stop condition:
  - if a real README/product defect is exposed during verification, create a bug immediately, ask for a task switch, and do not mark the task passed

## TDD Guardrails For Execution

- Every new test must fail before the supporting README or test-support change is added.
- Do not satisfy this task with grep-only checks scattered across multiple test files. One dedicated README content boundary must own the rules.
- Do not satisfy shortness by deleting required operator config or command examples.
- Do not keep contributor/process content in the README and merely hide it below the quick-start heading. The task requires it to be absent from the README.
- Do not swallow README-parse or contract failures.
- Do not run `make test-long` unless task execution genuinely changes ultra-long selection or explicitly requires that lane.

## Final Verification For The Execution Turn

- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] Do not run `make test-long` unless the task explicitly requires it or long-lane selection changes
- [ ] Final `improve-code-boundaries` pass confirms README content ownership is clean
- [ ] Update the task file checkboxes and set `<passes>true</passes>` only if no bug task was required

Plan path: `.ralph/tasks/story-24-readme-only-novice-e2e/02-task-verify-readme-stays-short-user-facing-and-inline-config-driven_plans/2026-04-20-readme-operator-surface-plan.md`

NOW EXECUTE
