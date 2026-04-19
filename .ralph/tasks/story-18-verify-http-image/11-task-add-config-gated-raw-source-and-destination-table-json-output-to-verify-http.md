## Task: Add config-gated raw source and destination table JSON output to verify HTTP <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Add a new verify HTTP feature that lets an operator query full raw table outputs for both CockroachDB and PostgreSQL in JSON form through the Go verify-service, with the capability explicitly enabled or disabled in the verify-service config. The higher order goal is to make the dedicated verify image usable as an operator-facing inspection surface for migration debugging without forcing operators to shell into databases or reconstruct mismatches indirectly.

Current product gap:
- the verify-service currently exposes only a narrow job-control HTTP surface
- the current public JSON no longer exposes detailed verify findings
- there is no supported operator path to ask the verify image for full raw source and destination table outputs in JSON when debugging a mismatch or validating completeness
- operators who need to inspect actual source-versus-destination data currently have to rely on database access or ad hoc test/log paths instead of the verify HTTP surface

In scope:
- add a verify-service config switch that explicitly enables or disables raw table-output querying
- keep the feature disabled by default unless the config enables it explicitly
- when enabled, expose an HTTP path or paths that let an operator request full raw JSON output for a selected table from the CockroachDB source and the PostgreSQL destination
- support returning raw rows in JSON regardless of the table’s column shape, as long as the values can be represented in JSON
- define clear request and response schemas for the raw table-output feature
- cover both source-side and destination-side output retrieval
- define loud failure behavior for unsupported, unreadable, or non-JSON-representable values instead of silently dropping fields
- preserve the existing config-owned connection boundary; HTTP callers must not be able to override DB URLs, TLS material, or verify modes

Out of scope:
- building a generic SQL query endpoint
- allowing arbitrary WHERE clauses, ORDER BY fragments, or free-form SQL from HTTP callers
- changing the main verify algorithm
- keeping backwards compatibility with the narrowed read surface if that blocks a cleaner operator-facing design

Decisions already made:
- this is a new feature, not just a bug fix
- raw source and destination table outputs must be JSON and operator-queryable through the Go verify-service
- the feature must be guarded by explicit config so deployments can enable or disable it intentionally
- the config gate should be the supported safety/control mechanism rather than relying on hidden test-only behavior
- if the feature is disabled in config, the HTTP surface must fail closed and not leak table contents

Relevant files:
- `cockroachdb_molt/molt/verifyservice/config.go`
- `cockroachdb_molt/molt/verifyservice/config_test.go`
- `cockroachdb_molt/molt/verifyservice/service.go`
- `cockroachdb_molt/molt/verifyservice/http_test.go`
- `cockroachdb_molt/molt/verifyservice/verify_runner.go`
- `cockroachdb_molt/molt/dbconn/*.go`

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers the config gate being disabled by default and explicitly enabled through verify-service config
- [ ] Red/green TDD covers HTTP retrieval of full raw JSON output for a selected CockroachDB source table and a selected PostgreSQL destination table when the feature is enabled
- [ ] The feature fails closed when disabled in config and does not expose raw table contents accidentally
- [ ] The feature preserves the config-owned connection boundary and does not allow HTTP callers to inject arbitrary connection or SQL details
- [ ] Raw table outputs are returned as JSON for arbitrary table shapes without silently dropping unsupported fields or errors
- [ ] The request and response schema for raw table-output querying is explicit and documented through tests
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
