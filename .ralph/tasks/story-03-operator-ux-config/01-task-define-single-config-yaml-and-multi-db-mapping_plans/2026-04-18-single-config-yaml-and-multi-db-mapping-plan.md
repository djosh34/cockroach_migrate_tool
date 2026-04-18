# Plan: Single Config YAML And Multi-Database Mapping

## References

- Task: `.ralph/tasks/story-03-operator-ux-config/01-task-define-single-config-yaml-and-multi-db-mapping.md`
- Design: `designs/crdb-to-postgres-cdc/02_requirements.md`
- Design: `designs/crdb-to-postgres-cdc/04_operational_model.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Design: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumption

- The task markdown plus the selected design docs are treated as the approval for this interface and behavior set.
- If the first execution slices prove that the public YAML shape is wrong, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Keep one operator-facing YAML file for the destination container. Do not introduce a second internal config format for runtime or grant generation.
- Replace the current single top-level `postgres` block with a `mappings` list. The container is global; destination PostgreSQL connection details are per mapping because one runner manages multiple destination databases.
- Keep source-side facts minimal inside the runner config. The runner needs source database identity and selected tables for routing, reconcile, and later verify/grant work; it does not need a source connection DSN here.
- Keep webhook, reconcile, and verify settings global because they apply to the one destination container process, not to an individual mapping.
- Keep PostgreSQL connection facts in one canonical validated shape reused everywhere. Do not duplicate host/port/database/user/password fields across raw parser types, runtime adapters, and summary renderers.
- Keep validation inside the private config parser boundary. Parsing and validation should produce fully validated config types once; later code should only consume validated types.
- Be bold with boundary cleanup during execution:
  - Split the current monolithic `crates/runner/src/config.rs` into a validated config module plus a private parser/validator module.
  - If `postgres.rs` remains only a label-copying adapter after the config redesign, remove it and render mapping summaries directly from validated config instead of maintaining fake runtime state.

## Public Contract To Establish

- `runner validate-config --config <path>` accepts one YAML file that describes:
  - one webhook listener with TLS material
  - one reconcile policy
  - one MOLT verify integration block for later use
  - one or more source-to-destination database mappings
- Each mapping has one stable mapping id plus:
  - `source.database`
  - `source.tables[]`
  - `destination.connection.host`
  - `destination.connection.port`
  - `destination.connection.database`
  - `destination.connection.user`
  - `destination.connection.password`
- The config models scoped PostgreSQL credentials only. No superuser role fields or privileged bootstrap assumptions belong in the runner config.
- `runner run --config <path>` reports a startup summary that proves the single container is wired with:
  - the config path
  - the webhook bind address and TLS files
  - the reconcile interval
  - the number of configured mappings
  - stable mapping labels derived from the canonical validated mapping type
- Validation rejects malformed operator input early:
  - zero mappings
  - empty mapping ids
  - duplicate mapping ids
  - empty source database names
  - empty table lists
  - duplicate table names within one mapping
  - empty destination connection fields
  - zero reconcile interval
  - empty webhook TLS paths

## Target YAML Shape

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
reconcile:
  interval_secs: 30
verify:
  molt:
    command: molt
    report_dir: /work/molt-verify
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.orders
    destination:
      connection:
        host: pg-a.internal
        port: 5432
        database: app_a
        user: migrator_a
        password: secret-a
  - id: app-b
    source:
      database: demo_b
      tables:
        - public.invoices
    destination:
      connection:
        host: pg-b.internal
        port: 5432
        database: app_b
        user: migrator_b
        password: secret-b
```

## Files And Structure To Add Or Change

- [x] `crates/runner/src/config.rs`
  - likely replaced by `crates/runner/src/config/mod.rs` plus a private parser module if that keeps validated shapes smaller and cleaner
- [x] `crates/runner/src/lib.rs`
- [x] `crates/runner/src/postgres.rs`
  - simplify heavily or delete if it still exists only to mirror validated config fields
- [x] `crates/runner/src/webhook.rs`
- [x] `crates/runner/src/reconcile.rs`
- [x] `crates/runner/tests/config_contract.rs`
- [x] `crates/runner/tests/long_lane.rs`
- [x] `crates/runner/tests/fixtures/valid-runner-config.yml`
- [x] `crates/runner/tests/fixtures/invalid-runner-config.yml`
- [x] `crates/runner/tests/fixtures/container-runner-config.yml`

## TDD Execution Order

### Slice 1: Tracer Bullet For Multi-Mapping Config

- [x] RED: replace the valid config fixture and add one integration-style assertion that `validate-config` accepts a YAML file with at least two mappings and reports a multi-mapping summary
- [x] GREEN: implement the smallest validated config redesign needed for one global container config plus a `mappings` list
- [x] REFACTOR: remove any leftover single-`postgres` naming that survives only as compatibility baggage

### Slice 2: Reject Structurally Invalid Mapping Lists

- [x] RED: add one failing contract test for the first high-value invalid case: empty `mappings` or duplicate mapping ids
- [x] GREEN: validate list cardinality and mapping-id uniqueness inside the config parser
- [x] REFACTOR: keep mapping collection validation owned by the mapping parser instead of scattered helper checks in `lib.rs` or runtime modules

### Slice 3: Reject Invalid Source Mapping Facts

- [x] RED: add one failing contract test for invalid source facts such as an empty source database or empty/duplicate table list
- [x] GREEN: add validated source-mapping types that normalize and reject bad source routing inputs
- [x] REFACTOR: make the source mapping type expose behavior-oriented accessors like stable mapping labels and table lists without leaking raw parse structs

### Slice 4: Canonical Destination Connection Shape

- [x] RED: add one failing contract test that proves destination connection validation and summary rendering occur through the mapping contract, not a single global `postgres` block
- [x] GREEN: replace the old `PostgresConfig` path with one canonical destination connection type nested under each mapping
- [x] REFACTOR: apply `improve-code-boundaries` smell 5 by using one render path for destination endpoint labels and removing duplicate string storage or duplicate field lists across config/runtime layers

### Slice 5: Global Webhook, Reconcile, And Verify Shape

- [x] RED: extend the valid-config and run-summary tests to require nested webhook TLS fields, non-zero reconcile interval, and a minimal verify block that can be consumed later
- [x] GREEN: add validated `WebhookConfig`, `ReconcileConfig`, and `VerifyConfig` shapes around the new YAML schema
- [x] REFACTOR: move raw YAML-only nested structs into a private parser module so the public validated config stays reduced and durable

### Slice 6: Runtime Summary Boundary Cleanup

- [x] RED: add one failing `run --config` contract assertion for stable output that includes mapping count and mapping labels from the new canonical config
- [x] GREEN: wire the startup summary from validated config and remove any fake runtime adapters that merely echo config strings
- [x] REFACTOR: apply `improve-code-boundaries` smells 11 and 12 by keeping config reduction inside the config module and deleting duplicate projection layers where possible

### Slice 7: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix the first failing lane only
- [x] GREEN: continue until every lane passes cleanly with the new multi-mapping contract
- [x] REFACTOR: do one final `improve-code-boundaries` pass and remove any stale single-database code, fixtures, or docs fragments uncovered by the refactor

## Boundary Review Checklist

- [x] No compatibility path keeps both `postgres` and `mappings` schemas alive
- [x] No validation of config structure happens outside the config parser boundary
- [x] No runtime adapter exists solely to copy mapping summary strings out of validated config
- [x] No separate config type exists for container, runtime, grant generation, or verify planning
- [x] No superuser-only PostgreSQL fields or assumptions appear in the new config schema
- [x] No single-database fixture or summary wording survives once the multi-mapping shape is live

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
