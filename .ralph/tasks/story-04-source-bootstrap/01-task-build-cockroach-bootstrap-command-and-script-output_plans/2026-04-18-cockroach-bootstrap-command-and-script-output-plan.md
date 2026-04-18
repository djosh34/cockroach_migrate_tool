# Plan: Cockroach Bootstrap Command And Script Output

## References

- Task: `.ralph/tasks/story-04-source-bootstrap/01-task-build-cockroach-bootstrap-command-and-script-output.md`
- Design: `designs/crdb-to-postgres-cdc/02_requirements.md`
- Design: `designs/crdb-to-postgres-cdc/04_operational_model.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Design: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Investigation: `investigations/cockroach-webhook-cdc/README.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumption

- The task markdown plus the selected design and investigation docs are treated as approval for this public contract and test surface.
- If the first execution slices prove that the public command shape or script contract is wrong, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Keep `crates/source-bootstrap` as the dedicated source-side CLI. Do not move Cockroach bootstrap script generation into `runner`.
- Replace the current stringly single-source `create-changefeed` placeholder with one operator-facing command that renders an executable bootstrap script:
  - `source-bootstrap render-bootstrap-script --config <path>`
- The command writes the full script to stdout. Operators can redirect it to a file or pipe it straight to `bash`; the binary itself does not execute source-side commands in this task.
- Render one shell script, not one vague summary line. The script must make source setup explicit and reproducible:
  - enable the required Cockroach cluster setting
  - capture the source cursor
  - create one changefeed per configured source database
  - print setup metadata after each changefeed is created
- Keep one global Cockroach connection URL and one global webhook sink URL in config, with one `mappings` list for per-database table selection. This keeps the operator mental model aligned with the runner config without copying destination-specific fields into source bootstrap.
- Apply `improve-code-boundaries` aggressively:
  - remove the current `BootstrapPlan` string bucket
  - keep config validation private to a `config/parser` boundary
  - introduce one typed script model that owns shell rendering instead of scattering `format!` calls through `lib.rs`
  - keep shell/sql quoting logic inside the rendering module, not mixed with config or clap wiring
- Keep emitted metadata and emitted SQL/script content derived from the same typed per-mapping plan so docs and executable output cannot drift.
- Do not add hidden post-setup source commands. The script produced here is the one explicit source-side operator action for CDC setup.

## Public Contract To Establish

- `source-bootstrap render-bootstrap-script --config <path>` accepts a typed YAML config describing:
  - one Cockroach SQL connection URL
  - one webhook sink URL
  - one resolved interval for changefeed progress messages
  - one or more source database mappings with stable ids and selected tables
- The emitted script is directly runnable in a pipeline and does not require the operator to invent missing Cockroach commands.
- The emitted script makes the required default-cluster prerequisite explicit:
  - `SET CLUSTER SETTING kv.rangefeed.enabled = true;`
- The emitted script captures an explicit starting cursor before creating changefeeds.
- The emitted script creates one changefeed per source database with the chosen contract:
  - explicit `cursor`
  - `initial_scan = 'yes'`
  - `envelope = 'enriched'`
  - `enriched_properties = 'source'`
  - `resolved = '<configured interval>'`
- Selected table filtering is explicit and per mapping. No changefeed may include tables outside the configured list.
- Multi-database setup is one operator flow: one invocation renders one script that bootstraps every configured mapping.
- The emitted script prints or persists the setup facts required by the design:
  - mapping id
  - source database
  - selected tables
  - starting cursor
  - created job id
- The command help must describe that this is a render step, not a hidden executor.

## Target Config Shape

```yaml
cockroach:
  url: postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require
webhook:
  url: https://runner.example.internal:8443/events
  resolved: 5s
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.orders
  - id: app-b
    source:
      database: demo_b
      tables:
        - public.invoices
```

## Target Script Shape

- Start with:
  - shebang
  - `set -euo pipefail`
- Capture one cluster cursor before creating feeds:
  - use `cockroach sql --url ...` inside the rendered script
  - persist the captured cursor in one shell variable reused by every changefeed block
- Make the required cluster setting executable in the script, not only documented in prose.
- Emit one deterministic block per mapping:
  - comment header with mapping id, database, and selected tables
  - `CREATE CHANGEFEED FOR TABLE ... INTO 'webhook-https://...' WITH ...`
  - capture and print the returned job id
  - print the mapping id, source database, and starting cursor for auditability
- Keep quoting deterministic for:
  - shell literals
  - SQL string literals
  - SQL identifiers for database and table names

## Files And Structure To Add Or Change

- [x] `crates/source-bootstrap/Cargo.toml`
  - add only the dependency changes actually needed for the rendered script or tests
- [x] `crates/source-bootstrap/src/lib.rs`
  - narrow clap wiring and command dispatch; no script assembly here
- [x] `crates/source-bootstrap/src/main.rs`
  - keep output and error handling explicit if rendering or stdout writes need a typed boundary
- [x] `crates/source-bootstrap/src/config.rs`
  - likely replace with `crates/source-bootstrap/src/config/mod.rs` plus a private parser module
- [x] `crates/source-bootstrap/src/render.rs`
  - new typed shell/script rendering module for bootstrap output
- [x] `crates/source-bootstrap/src/error.rs`
  - add explicit render/config/output errors; no string buckets and no swallowed failures
- [x] `crates/source-bootstrap/tests/bootstrap_contract.rs`
  - replace summary-line assertions with executable-script contract assertions
- [x] `crates/source-bootstrap/tests/cli_contract.rs`
  - assert the new subcommand/help surface
- [x] `crates/source-bootstrap/tests/fixtures/valid-source-bootstrap-config.yml`
  - replace the single-source placeholder fixture with a multi-mapping config
- [x] `crates/source-bootstrap/tests/fixtures/invalid-source-bootstrap-config.yml`
  - add one invalid fixture for high-value validation failures
- [x] `README.md`
  - document the source bootstrap flow alongside the runner quick start

## TDD Execution Order

### Slice 1: Tracer Bullet For Script Rendering

- [x] RED: replace the current bootstrap contract test with one integration-style CLI assertion that `source-bootstrap render-bootstrap-script --config <fixture>` emits a real shell script instead of `bootstrap plan ready`
- [x] GREEN: implement the smallest command/path change needed to load config and render a script header plus one mapping block
- [x] REFACTOR: delete the current `BootstrapPlan` summary struct and move rendering behind one typed output model

### Slice 2: Required Cluster Setting And Cursor Capture

- [x] RED: extend the contract test to require executable output for `kv.rangefeed.enabled` and explicit source cursor capture
- [x] GREEN: render the cluster-setting command and one reusable cursor-capture step into the script
- [x] REFACTOR: keep shell fragment rendering in one dedicated module so clap/config code never assembles command strings directly

### Slice 3: Changefeed SQL Contract

- [x] RED: add one failing assertion for the actual changefeed options and the selected-table list
- [x] GREEN: render `CREATE CHANGEFEED` with `cursor`, `initial_scan = 'yes'`, `envelope = 'enriched'`, `enriched_properties = 'source'`, and configured `resolved`
- [x] REFACTOR: centralize SQL literal and identifier quoting so there is one rendering path, not duplicate `format!` soup

### Slice 4: Multi-Database And Metadata Output

- [x] RED: add one failing test proving that two mappings produce two deterministic bootstrap blocks and print mapping/database/cursor/job metadata
- [x] GREEN: render one block per mapping and capture job ids from the `cockroach sql` invocation output
- [x] REFACTOR: derive human-readable metadata lines from the same typed per-mapping plan used for the executable block

### Slice 5: Config Validation Boundary

- [x] RED: add one failing contract test for invalid multi-mapping config such as zero mappings, duplicate ids, empty database names, or duplicate tables
- [x] GREEN: move all validation into a private config parser and produce fully validated config types only once
- [x] REFACTOR: apply `improve-code-boundaries` smell 12 so no extra validation or trimming survives in `lib.rs` or render code

### Slice 6: CLI Surface And Documentation

- [x] RED: add a failing CLI help assertion for `render-bootstrap-script` and a README contract assertion if needed through existing doc tests or file-content checks
- [x] GREEN: wire the new subcommand into help output and refresh README examples for the source-side flow
- [x] REFACTOR: remove any stale legacy wording that still implies a single-source summary-only command

### Slice 7: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix the first failing lane only
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass and remove any leftover dead types, duplicate quoting helpers, or placeholder bootstrap wording

## Boundary Review Checklist

- [x] No stringly `BootstrapPlan` summary type remains once real script output exists
- [x] No config validation survives outside the private config parser boundary
- [x] No shell/sql quoting logic is duplicated across clap, config, and rendering layers
- [x] No hidden source-side execution happens inside the generator command for this task
- [x] No changefeed output includes tables outside the configured selection
- [x] No single-source-only schema or fixture survives once multi-database bootstrap is implemented
- [x] No filesystem, parse, or rendering errors are swallowed or downgraded to plain strings

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
