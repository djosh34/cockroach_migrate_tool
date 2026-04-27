# Plan: Establish `docs/setup_sql` As The Canonical Bootstrap SQL Contract

## References

- Task:
  - `.ralph/tasks/story-28-replace-setup-sql-with-docs-and-bash-scripts/02-task-write-setup-sql-documentation.md`
- Current operator-facing entrypoints:
  - `README.md`
  - `docs/gpt_5_5_medium/getting-started.md`
- Public ingest path contract:
  - `crates/ingest-contract/src/lib.rs`
- Source-side SQL contract already exercised by the harness:
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/multi_mapping_harness.rs`
- Destination-side bootstrap and privilege contract:
  - `crates/runner/src/postgres_bootstrap.rs`
  - `crates/runner/tests/bootstrap_contract.rs`
  - `crates/runner/tests/reconcile_contract.rs`
- Design context for why operators do these steps:
  - `designs/crdb-to-postgres-cdc/04_operational_model.md`
  - `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Required skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This is a planning-only turn because the task file had no linked plan and no execution marker.
- This task is documentation work, so the TDD exception applies:
  - do not invent brittle string-match tests for markdown content
  - use the `tdd` mindset by validating the docs against the real public contract already exercised by the repo
  - prove the repo still stays green with the required command lanes after the docs change
- The docs must become the human-readable canonical boundary for setup SQL before Task 03 generates files from YAML.
- If execution shows that the scripts in Task 03 need a different SQL contract than the harness and runtime already enforce, this plan is wrong and must be switched back to `TO BE VERIFIED` immediately.
- If execution reveals an existing docs/navigation pattern that requires broader docs tree changes outside `docs/setup_sql/` to stay coherent, re-check scope before editing outside the task boundary.

## Current State Summary

- `docs/setup_sql/` does not exist yet.
- The bootstrap SQL contract is scattered across:
  - README prose that says operators must prepare SQL themselves
  - harness literals that render the exact CockroachDB `CREATE CHANGEFEED` statement
  - bootstrap and reconcile contract tests that show the minimum PostgreSQL grants the runtime role needs
- The current source-side contract from the harness is:
  - enable rangefeeds with `SET CLUSTER SETTING kv.rangefeed.enabled = true;`
  - capture a cursor with `USE <database>; SELECT cluster_logical_timestamp() AS changefeed_cursor;`
  - create a webhook changefeed with:
    - fully qualified table names
    - sink URL `webhook-https://<base>/<ingest path>?ca_cert=<encoded>`
    - `cursor = '<captured cursor>'`
    - `initial_scan = 'yes'`
    - `envelope = 'enriched'`
    - `enriched_properties = 'source'`
    - `resolved = '<interval>'`
- The current destination-side contract from bootstrap and reconcile tests is:
  - the runtime role must be able to connect to the destination database
  - the runtime role must be able to create objects in that destination database so it can create `_cockroach_migration_tool`
  - the runtime role must have `USAGE` on each mapped destination schema
  - the runtime role must have `SELECT, INSERT, UPDATE, DELETE` on each mapped real table
  - the runtime creates the helper schema `_cockroach_migration_tool` and its tracking tables itself
- The acceptance criteria name `{{ cockroach_url }}` as a documented variable even though the harness does not embed it into the `CREATE CHANGEFEED` SQL.
  - the docs should treat it as operator context and connection-target metadata shown in comments/examples, not as a SQL clause that changes runner behavior

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - the bootstrap SQL contract currently lives as scattered string literals and implied behavior across tests, design notes, and README fragments
- The docs task should create one explicit operator boundary:
  - `docs/setup_sql/index.md`
  - `docs/setup_sql/cockroachdb-source-setup.md`
  - `docs/setup_sql/postgresql-destination-grants.md`
- This is the right boundary cleanup because Task 03 can then generate SQL from the same documented contract instead of rediscovering it from test helpers.
- Bold cleanup rule for execution:
  - prefer one precise, canonical explanation over repeating half-explanations in multiple files
  - do not add compatibility prose for the deleted `setup-sql` binary
  - if a concept only matters to source setup or only to destination grants, keep it in that guide instead of duplicating it across both

## Public Verification Strategy

- No markdown-content unit tests.
- Execution should validate the docs against the repo's real public contract:
  - source SQL examples must match the shape exercised in `e2e_harness.rs` and `multi_mapping_harness.rs`
  - sink path examples must match `MappingIngestPath` from `ingest-contract`
  - destination grants must match the privileges proven in `bootstrap_contract.rs` and `reconcile_contract.rs`
  - helper schema wording must match `postgres_bootstrap.rs`
- Required command lanes before task completion:
  - `make check`
  - `make lint`
  - `make test`
- Optional spot checks during execution:
  - `rg -n "docs/setup_sql|setup_sql" README.md docs .ralph/tasks/story-28-replace-setup-sql-with-docs-and-bash-scripts`
  - targeted review of the new docs files to verify every placeholder from the acceptance criteria is documented exactly once

## Intended Files To Change

- Create:
  - `docs/setup_sql/index.md`
  - `docs/setup_sql/cockroachdb-source-setup.md`
  - `docs/setup_sql/postgresql-destination-grants.md`
- Update:
  - `.ralph/tasks/story-28-replace-setup-sql-with-docs-and-bash-scripts/02-task-write-setup-sql-documentation.md`
    - add the linked plan path
    - tick acceptance boxes only during execution, not in this planning turn

## Execution Slices

### Slice 1: Build The Canonical Docs Skeleton

- Create `docs/setup_sql/` and add `index.md`.
- `index.md` should provide:
  - a short explanation that operators prepare source SQL and destination SQL before running `runner`
  - a table of contents linking the two detailed guides
  - a quick-reference table showing:
    - which statements run on CockroachDB
    - which statements run on PostgreSQL
    - who runs them
    - when they are run
- RED signal:
  - the docs structure is still ambiguous and Task 03 would not know which guide owns which variables or SQL blocks
- GREEN:
  - the navigation makes the separation between source and destination explicit

### Slice 2: Write The CockroachDB Source Setup Guide

- Add `docs/setup_sql/cockroachdb-source-setup.md` with:
  - overview of why rangefeed enablement, one-time cursor capture, and changefeed creation are required
  - prerequisites section:
    - source cluster/client access
    - selected source database names and fully qualified tables
    - runner webhook base URL
    - CA certificate to trust the runner HTTPS endpoint
    - resolved interval choice
  - Jinja-style SQL template blocks using:
    - `{{ webhook_base_url }}`
    - `{{ ca_cert_base64 }}`
    - `{{ resolved_interval }}`
    - `{{ database }}`
    - `{{ schema }}`
    - `{{ table }}`
    - `{{ mapping_id }}`
    - `{{ cockroach_url }}`
  - clear cursor-capture notes:
    - capture once before creating the feed
    - paste the returned cursor into the later statement
    - explain why one captured starting point matters for initial scan and replay semantics
  - a multi-database, multi-mapping worked example that makes it obvious a per-database document/output may still contain multiple mapping-specific `CREATE CHANGEFEED` blocks
- RED signal:
  - the example SQL disagrees with the harness literals on path format, changefeed options, or table qualification
- GREEN:
  - a reader can trace every clause back to the live contract already exercised in the tests

### Slice 3: Write The PostgreSQL Destination Grants Guide

- Add `docs/setup_sql/postgresql-destination-grants.md` with:
  - overview of why the runtime role needs destination privileges
  - explanation that the runtime itself creates `_cockroach_migration_tool`, helper tables, and tracking tables
  - prerequisites section:
    - destination database already exists
    - runtime role already exists
    - real destination tables already exist in the mapped schemas
  - Jinja-style SQL template blocks using:
    - `{{ database }}`
    - `{{ runtime_role }}`
    - `{{ schema }}`
    - `{{ table }}`
  - explanation for each grant:
    - `CONNECT, CREATE` on database
    - `USAGE` on schema
    - `SELECT, INSERT, UPDATE, DELETE` on mapped real tables
  - annotated multi-database, multi-mapping example that shows deduplicated grants naturally when multiple mappings hit the same destination database/schema/role
- RED signal:
  - the docs imply extra privileges, omit required ones, or confuse real tables with helper schema ownership
- GREEN:
  - the guide matches the bootstrap contract exactly and explains why those privileges are sufficient

### Slice 4: Contract Review And Repo Validation

- Verify every acceptance-criteria placeholder is documented:
  - `{{ webhook_base_url }}`
  - `{{ ca_cert_base64 }}`
  - `{{ resolved_interval }}`
  - `{{ database }}`
  - `{{ schema }}`
  - `{{ table }}`
  - `{{ mapping_id }}`
  - `{{ runtime_role }}`
  - `{{ cockroach_url }}`
- Manually compare the final docs examples to:
  - `MappingIngestPath`
  - the harness `CREATE CHANGEFEED` strings
  - the destination bootstrap/grant strings in the tests
- Run:
  - `make check`
  - `make lint`
  - `make test`
- Final mud check using `improve-code-boundaries`:
  - confirm `docs/setup_sql/` now serves as the only explicit bootstrap SQL contract boundary
  - confirm the new docs do not duplicate or contradict the source of truth already enforced by the code/tests
- If command lanes fail for a reason that means the docs contract is still underspecified or wrong, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Done Condition

- `docs/setup_sql/index.md` exists and cleanly routes readers to the source and destination guides.
- The CockroachDB guide explains the exact setup SQL contract and cursor workflow in operator language, with Jinja templates and a worked example.
- The PostgreSQL guide explains the exact runtime-role grant contract, with Jinja templates and a worked example.
- The new docs make Task 03 implementation straightforward because the SQL contract is explicit instead of scattered.
- The repo passes `make check`, `make lint`, and `make test`.

Plan path: `.ralph/tasks/story-28-replace-setup-sql-with-docs-and-bash-scripts/02-task-write-setup-sql-documentation_plans/2026-04-27-setup-sql-docs-plan.md`

NOW EXECUTE
