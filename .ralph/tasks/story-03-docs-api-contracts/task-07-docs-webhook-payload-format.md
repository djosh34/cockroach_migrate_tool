## Task: Document runner webhook payload format for API consumers <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete

**Goal:** Add clear, complete documentation of the runner webhook payload format so that users setting up CockroachDB changefeeds or custom webhook producers do not need to read Rust source code. Currently the README only lists `POST /ingest/<mapping_id>` with no request body documentation. The only specification is in `crates/runner/src/webhook_runtime/payload.rs`.

**Exact things to include:**
- A new dedicated section in the README under "Runner Quick Start" titled "Webhook Payload Format".
- Complete JSON example of a **row-batch** payload with at least 2 rows (one upsert, one delete).
- Complete JSON example of a **resolved** payload.
- Field-by-field description table: field name, type, required/optional, description.
- List of valid `op` values with meanings: `c` (create/insert), `u` (update), `r` (refresh/upsert), `d` (delete).
- Description of `source` object fields: `database_name`, `schema_name`, `table_name`.
- Description of `key` and `after` objects (arbitrary JSON objects mapping column names to values).
- Explanation that all rows in a batch must belong to the same source table.
- Explanation that `length` must match `payload` array length.
- A curl example showing a complete `POST /ingest/app-a` request.
- HTTP response codes: 200 OK, 400 Bad Request (with example error body), 404 Unknown Mapping, 500 Internal Server Error.

**Exact things NOT to include:**
- Internal Rust struct names (`RowBatchRequest`, `RowEvent`, `RowMutation`).
- Implementation details about routing, persistence, or SQL generation.
- References to `crates/runner/src/webhook_runtime/payload.rs` or other source files.
- CockroachDB CDC setup instructions (those belong in setup-sql docs).
- Prometheus metrics details.
- Authentication or TLS setup (already covered elsewhere).
- Advanced payload shapes not supported by the current parser.

**End result:**
A user can scroll to the "Webhook Payload Format" section in the README and immediately construct a valid `curl` command to test the runner without reading any source code.
</description>

<acceptance_criteria>
- [ ] README contains a "Webhook Payload Format" section with complete examples
- [ ] Row-batch example includes all required fields and shows valid `op` values
- [ ] Resolved example is complete and distinct from row-batch
- [ ] Field description table covers all top-level and nested fields
- [ ] curl example is copy-pasteable and targets `/ingest/<mapping_id>`
- [ ] Response codes are documented with example bodies
- [ ] No internal Rust source code references appear in the documentation
- [ ] README operator surface contract test passes (word count, heading structure)
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
