# Plan: Verify-Service Structured Multi-Database Config

## References

- Task:
  - `.ralph/tasks/story-35-verify-multi-database-config/01-task-verify-multi-db-config.md`
- Current verify-service config and runtime seams:
  - `cockroachdb_molt/molt/verifyservice/config.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner.go`
  - `cockroachdb_molt/molt/verifyservice/raw_table.go`
  - `cockroachdb_molt/molt/verifyservice/filter.go`
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
- Current tests to extend through public interfaces:
  - `cockroachdb_molt/molt/verifyservice/config_test.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner_test.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
- Operator docs and schema references:
  - `docs/operator-guide/config-reference.md`
  - `docs/operator-guide/verify-service.md`
  - `openapi/verify-service.yaml`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown already approves the operator-facing direction for this turn.
- This turn is planning-only because the task file had no execution marker when opened.
- This task is the config and single-request execution foundation.
  - It may add the minimum job/raw-table selector needed to target one configured database.
  - It must not implement the broader multi-job, globbed, all-databases HTTP UX reserved for Task 2.
- No backwards compatibility is allowed.
  - Remove the old operator-facing `verify.source.url` and `verify.destination.url` shape instead of supporting both.
- Config loading must continue to fail closed with known-field YAML decoding and explicit operator errors.
- If the first RED slice proves the public contract below cannot be implemented cleanly without redesigning Task 2 at the same time, execution must switch this plan back to `TO BE VERIFIED` and stop immediately.

## Current State Summary

- `verifyservice.Config` is single-pair oriented.
  - `VerifyConfig` has exactly one `Source` and one `Destination`.
  - `DatabaseConfig` is really an operator-facing connection URL wrapper plus TLS attachments.
- `VerifyRunner` always resolves one source connection string and one destination connection string directly from top-level config.
- Raw-table reading is also single-pair oriented.
  - `RawTableRequest.Database` is misnamed: it means endpoint side (`source|destination`), not configured database mapping.
  - There is no selector for which configured source/destination database pair should be used.
- HTTP job requests currently carry only filters, not a configured database selector.
- Command validation summaries still assume one source SSL mode and one destination SSL mode.

## Boundary Problem To Flatten

- The main boundary smell is that one type, `DatabaseConfig`, currently mixes three different concerns:
  - operator-facing connection input shape
  - internal connection-string construction
  - single-pair runtime assumptions
- A second naming smell compounds it:
  - `RawTableRequest.Database` sounds like a configured database name, but it actually means source/destination side
- Execution should flatten this by introducing one explicit resolution boundary:
  - operator-facing config types describe defaults and per-database overrides
  - one resolver produces a typed internal `ResolvedDatabasePair`
  - runtime, raw-table, and command reporting consume resolved pairs instead of rebuilding assumptions ad hoc
- This is the `improve-code-boundaries` requirement for the task.
  - Do not keep parallel shapes where one path uses raw URLs and another path uses structured fields.
  - Do not let service, runner, and raw-table each merge defaults independently.

## Public Contract To Establish

- `verify.source` and `verify.destination` become optional default connection blocks.
- `verify.databases` becomes required and contains only mapping objects.
  - Scalar entries such as `- app` are rejected.
- Each `verify.databases[]` entry has:
  - `name`
  - either:
    - `source_database` and `destination_database` plus optional per-side overrides
    - or fully specified `source` and `destination` connection blocks that include `database`
- Defaults may provide shared source and destination endpoint settings.
- Per-database `source` and `destination` blocks may override defaults field-by-field.
- Unsupported passthrough fields such as `params` and `application_name` are rejected by known-field decoding.
- Duplicate configured database names are rejected.
- Validation must happen after defaults and overrides are merged, so TLS and required-field checks see the effective connection settings.
- Verify execution continues to invoke MOLT with exactly two ordered connections per run.
- HTTP job requests gain the minimum selector needed for this task:
  - a single configured database name field, not a globbed multi-database UX
- Raw-table requests gain the minimum selector needed for this task:
  - configured database name
  - endpoint side (`source|destination`)
- If multiple databases are configured and the request omits the configured database selector, the API rejects the request as ambiguous with a clear error.

## Proposed Type Shape

- Replace the current URL-oriented operator config types with structured connection input types:
  - `ConnectionDefaults`
  - `ConnectionOverride`
  - `DatabaseMappingConfig`
  - `DatabaseEndpointConfig`
  - `DatabaseTLSConfig`
- Add one internal resolved type:
  - `ResolvedDatabasePair`
    - `Name`
    - `Source`
    - `Destination`
- Add one resolved endpoint type that owns safe URL construction:
  - `ResolvedConnection`
    - host
    - port
    - database
    - user
    - password file or password material reference already used by the existing URL builder contract
    - sslmode
    - tls material
    - `ConnectionString()`
- Keep connection-string building centralized in the resolved endpoint layer using `net/url`.
  - No string concatenation.
- Rename the raw-table side enum to reflect reality.
  - Example direction:
    - `RawTableSide`
    - values `source` and `destination`
- Extend runtime request types minimally:
  - `JobRequest.Database string`
  - `RunRequest.Database string`
  - `RawTableRequest.Database string` becomes the configured database name
  - `RawTableRequest.Side RawTableSide`

## Validation Rules To Encode

- Listener validation remains as-is.
- Config validation must additionally enforce:
  - `verify.databases` is present and non-empty
  - every entry is a mapping object
  - every entry has a unique `name`
  - every resolved source and destination has required fields:
    - host
    - port
    - database
    - user
    - sslmode if the contract requires explicitness after refactor
  - `verify-ca` and `verify-full` require a CA certificate after merge
  - client cert and client key must appear together after merge
- Error messages should identify the effective field path where practical:
  - `verify.databases[1].source.user`
  - `verify.databases[2].destination.tls.ca_cert_path`
  - `verify.databases[0].name`
- Duplicate names should name the duplicate configured database explicitly.

## Minimal HTTP Surface For This Task

- `POST /jobs`
  - accepts an optional `database` field naming one configured database
  - if exactly one configured database exists, omission may default to that database
  - if multiple configured databases exist, omission is rejected as ambiguous
- `POST /tables/raw`
  - accepts configured `database` name plus `side`
  - if the old request shape is still present, remove it rather than preserve compatibility
- Task 2 will redesign the broader jobs UX later.
  - Do not add globs, `databases` arrays, or multi-job execution here.
  - The goal here is only to make the multi-database config executable through one selected mapping.

## TDD Slices

### Slice 1: Tracer Bullet For Default Credentials Plus Per-Database Names

- RED:
  - add a config-loading test that uses the first required YAML example
  - assert:
    - config loads
    - one selected configured database resolves to the expected source and destination connection strings
    - defaults are inherited into multiple database mappings
    - old URL fields are not part of the config contract
- GREEN:
  - introduce the new structured config types
  - add the resolver that merges defaults with one database mapping
  - build connection strings through the resolved endpoint type
- REFACTOR:
  - remove any leftover single-pair URL helpers that no longer belong at the operator boundary

### Slice 2: Per-Database Override Inheritance

- RED:
  - add a config-loading test for the second required YAML example
  - assert:
    - inherited defaults remain intact
    - per-database source and destination overrides replace only the requested fields
    - resolved connection strings reflect the audit override credentials and default hosts/TLS
- GREEN:
  - implement field-by-field override merge
- REFACTOR:
  - keep merge logic in one resolver module, not scattered across validation and execution

### Slice 3: Fully Specified Per-Database Connections Without Defaults

- RED:
  - add a config-loading test for the third required YAML example
  - assert:
    - config loads without top-level default `verify.source` / `verify.destination`
    - each database entry resolves independently
    - destination database names may differ from source database names
- GREEN:
  - support fully specified per-database source/destination blocks
- REFACTOR:
  - ensure defaultless and inherited cases share one resolution path

### Slice 4: Rejections For Unsupported Input Shapes

- RED:
  - add config tests that prove:
    - scalar database entries are rejected
    - duplicate names are rejected
    - unsupported fields like `params` and `application_name` are rejected
    - missing inherited fields produce field-specific errors
- GREEN:
  - tighten YAML decoding and validation around the new schema
- REFACTOR:
  - keep operator error construction explicit and avoid lossy wrapping

### Slice 5: TLS Validation After Merge

- RED:
  - add tests where defaults plus overrides combine into invalid effective TLS:
    - `verify-full` without CA after merge
    - `verify-ca` without CA after merge
    - client cert without client key after merge
  - assert errors name the effective merged path
- GREEN:
  - validate resolved endpoints rather than partially merged input fragments
- REFACTOR:
  - keep TLS rules in the resolved endpoint validation layer

### Slice 6: Command Validation JSON Logging Uses Effective Multi-Database Summary

- RED:
  - extend `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - assert invalid structured configs logged via `validate-config --log-format json` retain:
    - `category`
    - `code`
    - `message`
    - `details`
  - assert details identify the bad structured field
  - assert success output no longer assumes one source/destination summary only
- GREEN:
  - adapt command validation reporting to the new config shape
- REFACTOR:
  - keep log summaries derived from the resolved database inventory, not from obsolete single-pair fields

### Slice 7: Verify Runner Resolves One Selected Configured Database Before Connecting

- RED:
  - extend `verify_runner_test.go`
  - assert a request naming a configured database causes the runner to connect with that mapping's resolved source and destination strings
  - assert requesting an unknown database returns a clear operator error
  - assert omission with multiple configured databases fails as ambiguous
- GREEN:
  - make `VerifyRunner` resolve `RunRequest.Database` through the config resolver
- REFACTOR:
  - keep connection-pair lookup in one helper shared by runner and raw-table code

### Slice 8: Raw-Table Requests Require An Unambiguous Configured Database Selector

- RED:
  - extend `http_test.go` and `raw_table.go` behavior through the public HTTP handler
  - assert:
    - when multiple configured databases exist, omitting the configured database selector is rejected clearly
    - specifying configured `database` plus `side` reaches the expected source or destination connection pair
    - invalid side values are rejected clearly
- GREEN:
  - update the raw-table request schema and config-backed reader to resolve one configured database pair first
- REFACTOR:
  - rename the misleading raw-table enum and delete any compatibility shim that preserves the old ambiguous meaning

### Slice 9: Docs And OpenAPI Move To The New Canonical Shape

- RED:
  - add or update doc-oriented verification only through real project checks, not brittle string-matching unit tests
  - rely on `make check`, `make lint`, and `make test` as the guardrails after docs/schema edits
- GREEN:
  - replace old single-URL examples with the three task examples
  - update OpenAPI request/response examples for the minimal database selector introduced here
- REFACTOR:
  - remove obsolete single-pair config examples and references entirely

## Execution Order

- Execute slices strictly in vertical red/green order.
- Start with config parsing and resolution because runner, raw-table, CLI output, docs, and HTTP all depend on that boundary.
- Only after the resolver is stable should the runner and raw-table paths switch over.
- Leave broader HTTP UX redesign for Task 2.

## Verification Gates

- Required before marking the task done:
  - `make check`
  - `make lint`
  - `make test`
- Do not run `make test-long` for this task unless execution proves this task explicitly moved ultra-long coverage.
- Final review must include one explicit `improve-code-boundaries` pass:
  - check that no obsolete single-pair config DTOs or ambiguous `database`/`side` naming remain
  - remove dead code instead of leaving compatibility wrappers

## Switch-Back Conditions

- Switch this plan back to `TO BE VERIFIED` immediately if:
  - the minimal single-database selector needed for Task 1 cannot be added without locking in the wrong HTTP UX for Task 2
  - MOLT verify invocation unexpectedly requires a config shape broader than one resolved source/destination pair per run
  - the resolved endpoint model reveals a cleaner contract that invalidates the field layout above
  - raw-table semantics need a fundamentally different API than `database + side` to avoid operator confusion

Plan path: `.ralph/tasks/story-35-verify-multi-database-config/01-task-verify-multi-db-config_plans/2026-04-29-verify-multi-db-config-plan.md`

NOW EXECUTE
