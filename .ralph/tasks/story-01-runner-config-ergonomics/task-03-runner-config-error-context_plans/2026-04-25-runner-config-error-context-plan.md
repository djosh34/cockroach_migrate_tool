# Plan: Improve Runner Config Validation Errors With Actual Values

## References

- Task:
  - `.ralph/tasks/story-01-runner-config-ergonomics/task-03-runner-config-error-context.md`
- Current validation boundary:
  - `crates/runner/src/config/parser.rs`
  - `crates/runner/src/error.rs`
- Current public contract coverage:
  - `crates/runner/tests/config_contract.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient product approval for this planning turn.
- This turn is planning-only because task 03 had no existing plan artifact.
- This is greenfield work.
  - improve the error text directly instead of preserving vague legacy wording
  - keep the existing `RunnerConfigError` variants unless the first RED slice proves otherwise
  - prefer deleting duplicated error-string construction over adding more helper sprawl
- The current `RunnerConfigError::InvalidFieldDetail` variant is likely sufficient for all in-scope message upgrades.
- If the first RED slice proves the parser needs a new error shape rather than using the existing detail variant, switch this plan back to `TO BE VERIFIED` and stop immediately.
- If the first RED slice proves that value-aware messaging would force multiple competing formatting helpers across modules, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - invalid source table names echo the actual bad value and the expected `schema.table` shape
  - duplicate mapping IDs echo the duplicated ID
  - duplicate source tables echo the duplicated table name
  - empty or whitespace-only string fields say the field was empty or whitespace
  - empty `mappings` explains that at least one mapping is required
- Lower-priority concerns:
  - preserving the exact old static message text
  - expanding validation to new fields or new rules beyond the task

## Current State Summary

- `parser.rs` already has the needed public validation seams:
  - `validate_mappings`
  - `validate_tables`
  - `validate_table_name`
  - `validate_text`
- Those helpers currently return static `InvalidField` messages for the behaviors in scope:
  - duplicate IDs: `must be unique`
  - duplicate tables: `must not contain duplicates`
  - invalid table names: `entries must use schema.table`
  - empty strings: `must not be empty`
  - empty mappings: `must contain at least one mapping`
- `RunnerConfigError::InvalidFieldDetail` already exists and can carry dynamic message text without changing the public error surface.
- `config_contract.rs` already has coverage for:
  - duplicate IDs
  - duplicate source tables
  - unqualified source tables
- `config_contract.rs` does not yet cover:
  - empty mappings with the more descriptive wording
  - empty/whitespace string-field messaging

## Boundary Decision

- Keep one config error model.
  - do not add new config-specific error enums for this task
  - use `InvalidFieldDetail` for value-aware messaging
- Keep the parser helper layer small and honest.
  - validation helpers should decide the field and the normalized value
  - one small message-formatting seam should build the dynamic detail text
- Do not spread inline `format!` calls across every validation branch if one helper can own the repeated wording.

## Improve-Code-Boundaries Focus

- Primary smell to flatten:
  - parser validation helpers mix domain checks with repeated ad hoc message phrasing
- Required cleanup during execution:
  - centralize dynamic invalid-field message construction behind one narrow helper or a few tight helper functions inside `parser.rs`
  - keep `validate_mappings`, `validate_tables`, `validate_table_name`, and `validate_text` focused on validation logic instead of each inventing its own formatting style
  - avoid adding a second parallel family of static and dynamic error helpers
- Bold refactor allowance:
  - if `InvalidField` becomes unused for the touched parser paths after the refactor, stop using it there rather than preserving mixed styles
  - if one helper can remove repeated `RunnerConfigError::InvalidFieldDetail { ... }` blocks, add it and delete the duplication

## Error Contract Decisions

- Invalid table name message target:
  - field remains `mappings.source.tables`
  - message should include the actual invalid value and an example such as `public.customers`
- Duplicate mapping ID message target:
  - field remains `mappings.id`
  - message should say `duplicate value "<id>"`
- Duplicate source table message target:
  - field remains `mappings.source.tables`
  - message should say `duplicate value "<schema.table>"`
- Empty or whitespace-only string message target:
  - keep the current field path from the caller
  - message should say the value was empty or whitespace, not just “must not be empty”
- Empty mappings message target:
  - field remains `mappings`
  - message should explicitly say the config must declare at least one mapping

## Intended Files And Structure To Add Or Change

- `crates/runner/src/config/parser.rs`
  - update validation helpers to emit value-aware detail messages
  - add one focused helper for dynamic invalid-field text if it reduces duplication
- `crates/runner/tests/config_contract.rs`
  - update existing assertions for improved error text
  - add new contract tests for empty mappings and whitespace-only field values
- `crates/runner/src/error.rs`
  - no change expected
  - only touch if the execution turn proves the existing detail variant is insufficient

## Public Contract Decisions

- Representative failure shapes after execution:

```text
config: invalid config field `mappings.source.tables`: found "customers", expected schema-qualified name like "public.customers"
```

```text
config: invalid config field `mappings.id`: duplicate value "app-a"
```

```text
config: invalid config field `mappings.source.database`: value was empty or whitespace
```

```text
config: invalid config field `mappings`: must define at least one mapping
```

- Exact wording may vary slightly during RED/GREEN if the contract tests settle on a clearer phrase, but the public contract must preserve:
  - field-specific failures
  - actual bad value for duplicate or malformed table input
  - explicit empty/whitespace language for blank strings

## Vertical TDD Slices

### Slice 1: Tracer Bullet For Invalid Table Name Context

- RED:
  - tighten the existing unqualified-table contract test in `config_contract.rs`
  - require the failure to include the actual invalid value and an example of the expected format
- GREEN:
  - update `validate_table_name` to emit a dynamic detail message
- REFACTOR:
  - move the repeated dynamic invalid-field construction behind one small helper if the first GREEN introduces duplication

### Slice 2: Duplicate Mapping ID Context

- RED:
  - tighten the duplicate-ID contract test to require the duplicated ID string
- GREEN:
  - update `validate_mappings` to emit a dynamic duplicate-value message
- REFACTOR:
  - keep duplicate-value wording shared with the source-table duplicate path if the phrasing matches

### Slice 3: Duplicate Source Table Context

- RED:
  - tighten the duplicate-source-table contract test to require the duplicated table name
- GREEN:
  - update `validate_tables` to emit a dynamic duplicate-value message
- REFACTOR:
  - remove any duplicated “duplicate value” formatting logic introduced between slices 2 and 3

### Slice 4: Empty Or Whitespace String Context

- RED:
  - add one failing contract test that uses a whitespace-only string field through the public config surface
  - require the failure message to say the value was empty or whitespace
- GREEN:
  - update `validate_text` to emit the improved empty-value detail
- REFACTOR:
  - keep the helper generic so all string-backed config fields automatically benefit without per-field message branches

### Slice 5: Empty Mappings Context

- RED:
  - add one failing contract test for an empty `mappings: []` config
  - require the more descriptive empty-mappings wording
- GREEN:
  - update `validate_mappings` empty-list handling
- REFACTOR:
  - ensure the message stays specific to the collection boundary instead of becoming a generic empty-value error

### Slice 6: Final Lanes And Boundary Pass

- RED:
  - after the behavior slices are green, run:
    - `make check`
    - `make lint`
    - `make test`
  - do not run `make test-long`
- GREEN:
  - continue until every required default lane passes
- REFACTOR:
  - do one final `improve-code-boundaries` pass and confirm the parser validation boundary got simpler rather than more stringly

## TDD Guardrails For Execution

- One failing behavior slice at a time.
- Do not add tests after implementation for the same behavior.
- Test through the public runner surface first:
  - `runner validate-config --config ...`
- Do not add new validation rules that were not requested.
- Do not change field paths in the emitted errors.
- Do not swallow original values when the message is supposed to be value-aware.
- If execution discovers that dynamic message construction is spreading into multiple modules instead of staying local to parser validation, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Final Verification For The Execution Turn

- [x] Invalid table name errors include the actual invalid value and expected format example
- [x] Duplicate mapping ID errors include the duplicated ID string
- [x] Duplicate table errors include the duplicated table name
- [x] Empty/whitespace field errors indicate the field was empty or whitespace
- [x] Empty mappings errors explain that at least one mapping is required
- [x] Existing config contract tests still pass with updated wording
- [x] New tests assert on improved error message content
- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long` not run because this task does not require the long lane
- [x] Final `improve-code-boundaries` pass confirms the parser validation boundary got simpler rather than muddier
- [x] Update the task file and set `<passes>true</passes>` only after all required lanes pass

Plan path: `.ralph/tasks/story-01-runner-config-ergonomics/task-03-runner-config-error-context_plans/2026-04-25-runner-config-error-context-plan.md`

NOW EXECUTE
