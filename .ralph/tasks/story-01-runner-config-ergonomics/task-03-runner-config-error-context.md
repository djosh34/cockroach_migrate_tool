## Task: Improve runner config validation error messages with actual values <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete

**Goal:** Update config validation error messages in `crates/runner/src/config/parser.rs` to include the actual values the user provided, making errors immediately actionable. Currently errors like `entries must use schema.table` or `must be unique` do not tell the user what they typed wrong.

**In scope:**
- Update `validate_table_name` to report the actual invalid value (e.g., `"found 'customers', expected 'schema.table' format like 'public.customers'"`).
- Update duplicate ID detection to report the duplicated ID value.
- Update duplicate table detection to report the duplicated table name.
- Update empty string validation to report the field name and indicate it was empty/whitespace.
- Update `validate_mappings` empty check to be more descriptive.
- Keep all existing error variants in `RunnerConfigError` (do not add new ones unless needed for formatting).
- Update tests in `crates/runner/tests/config_contract.rs` to assert on the improved messages.

**Out of scope:**
- Adding new validation rules.
- Changing error types or error enum structure.
- Improving errors outside of config parsing (e.g., runtime errors, webhook errors).
- Changing non-runner config errors (setup-sql errors are out of scope).

**End result:**
Instead of:
```
config: invalid config field `mappings.source.tables`: entries must use schema.table
```
Users see:
```
config: invalid config field `mappings.source.tables`: found "customers", expected schema-qualified name like "public.customers"
```

And instead of:
```
config: invalid config field `mappings.id`: must be unique
```
Users see:
```
config: invalid config field `mappings.id`: duplicate value "app-a"
```
</description>

<acceptance_criteria>
- [ ] Invalid table name errors include the actual invalid value and expected format example
- [ ] Duplicate mapping ID errors include the duplicated ID string
- [ ] Duplicate table errors include the duplicated table name
- [ ] Empty/whitespace field errors indicate the field was empty
- [ ] All existing config contract tests still pass (messages may be updated)
- [ ] New tests assert on the improved error message content
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
