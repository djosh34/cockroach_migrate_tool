# Deep Ergonomics Investigation: New User Experience

**Date:** 2026-04-25
**Scope:** Configuration system, HTTP/API surfaces, CLI verbs, onboarding friction
**Method:** Walkthrough as a novice operator, reading code, tests, README, and config schemas.

---

## Executive Summary

The project has a clean operator-facing README and a well-tested public surface, but the **config system is fragmented across three incompatible schemas**, the **HTTP APIs have mismatched design philosophies**, and several **sharp edges** will trip up new users before they get a migration running. The biggest risks are: secrets-in-YAML, no HTTP fallback for local dev, and the cognitive load of maintaining three config files for one pipeline.

---

## 1. Configuration System

### 1.1 There Are Three Config Schemas for One Pipeline

A migration requires:

1. `setup-sql` **cockroach-setup** config (`cockroach.url`, `webhook.base_url`, `mappings`)
2. `setup-sql` **postgres-grants** config (`mappings[].destination.database`, `runtime_role`, `tables`)
3. `runner` config (`webhook.bind_addr`, `reconcile.interval_secs`, `mappings[].source`, `mappings[].destination`)
4. `verify-service` config (`listener.bind_addr`, `verify.source`, `verify.destination`)

All four have different shapes. A user must duplicate mapping IDs, database names, and table lists across files. There is no single source of truth.

**Example of duplication pain:**

```yaml
# cockroach-setup.yml
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.orders

# runner.yml
mappings:
  - id: app-a          # same ID, manually kept in sync
    source:
      database: demo_a  # same database
      tables:
        - public.customers
        - public.orders
    destination:
      host: pg-a.example.internal
      # ...
```

If the user changes a table list in one file but not the other, the system will not catch it. The runner validates that tables use `schema.table` format, but it does not cross-check against the cockroach-setup config because those are two separate commands.

### 1.2 TLS Is Configured Inconsistently Between Components

| Component | Server TLS | Client TLS (DB) | Where `sslmode` lives |
|-----------|------------|-----------------|----------------------|
| Runner | `webhook.tls.cert_path` + `key_path` (mandatory) | `mappings[].destination.tls.mode` (`require`/`verify-ca`/`verify-full`) | YAML field |
| Verify (Go) | `listener.tls.cert_path` + `key_path` (optional) | `verify.source.url` query param `sslmode` | URL query string |

A new user must learn two different mental models:
- Runner: `mode: verify-ca` + `ca_cert_path`
- Verify: `url: postgresql://...?sslmode=verify-ca` + `ca_cert_path`

This is especially confusing because the runner *destination* uses a YAML enum for SSL mode, while verify uses a URL query parameter for the same concept. The README even has to call this out explicitly: "Database URLs own `sslmode`; the YAML only carries mounted cert paths."

### 1.3 Runner Webhook Listener Is HTTPS-Only

```rust
// crates/runner/src/webhook_runtime/mod.rs:56
let tls_acceptor = TlsAcceptor::from(Arc::new(load_tls_config(runtime.as_ref())?));
```

There is no `mode: http` option. For local development, testing, or internal trusted networks, a user must still generate a self-signed cert and mount it. This adds friction to the "hello world" experience.

Compare with verify-service, which has a natural HTTP fallback when `listener.tls` is omitted.

### 1.4 Secrets Are Stored in Plain Text in YAML

```yaml
# runner.yml
mappings:
  - id: app-a
    destination:
      password: runner-secret-a   # plain text, no env substitution
```

The runner config parser (`crates/runner/src/config/parser.rs`) reads the file directly with `fs::read_to_string`. There is no `${ENV_VAR}` interpolation, no Kubernetes secret reference, no file-based secret indirection. The password sits in the same file as the table mappings.

This means:
- The config file cannot be checked into Git safely.
- The config file cannot be stored in a ConfigMap without exposing credentials.
- The user must manage file permissions manually.

The `setup-sql` cockroach URL and the verify-service DB URLs have the same problem: credentials are embedded in the YAML or URL string.

### 1.5 Runner Destination Uses Decomposed Fields Instead of Connection String

```yaml
# What the user must write
destination:
  host: pg-a.example.internal
  port: 5432
  database: app_a
  user: migration_user_a
  password: runner-secret-a
  tls:
    mode: verify-ca
    ca_cert_path: /config/certs/destination-ca.crt
    client_cert_path: /config/certs/destination-client.crt
    client_key_path: /config/certs/destination-client.key
```

Most PostgreSQL tools accept a connection string:
```
postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a?sslmode=verify-ca&sslrootcert=...
```

The decomposed format is more verbose and unfamiliar to anyone who has used `psql`, `sqlx`, or any major PostgreSQL client. It also prevents users from copy-pasting a working connection string from their existing tooling.

### 1.6 Setup-SQL Cockroach Config Reads the CA Cert at Parse Time

```rust
// crates/setup-sql/src/config/cockroach_parser.rs:55-63
let ca_cert_path = resolve_config_path(...);
let ca_cert_bytes = fs::read(&ca_cert_path).map_err(...)?;
```

The `setup-sql emit-cockroach-sql` command reads the CA certificate file during config validation and base64-encodes it into the emitted SQL. This is a surprising side effect: the user expects config validation to check syntax and paths, but instead it reads file contents and mutates them into query parameters. If the CA cert changes after SQL emission, the changefeed sink URL in the emitted SQL is now stale.

### 1.7 Good Validation, But Error Messages Could Be More Actionable

The runner has excellent validation:
- `deny_unknown_fields` on YAML parsing
- Duplicate mapping ID detection
- Duplicate table detection within a mapping
- `schema.table` format enforcement
- Nonempty string checks

Error example:
```
config: invalid config field `mappings.source.tables`: entries must use schema.table
```

This is decent, but it does not tell the user *what* they wrote instead. If they wrote `customers`, the error does not say "found `customers`, expected `schema.table`". Similarly, `mappings.id: must be unique` does not say which ID was duplicated.

### 1.8 Verify-Service Config Has a Subtle URL Parsing Trap

```go
// cockroachdb_molt/molt/verifyservice/config.go:126-132
func (cfg DatabaseConfig) SSLMode() string {
    parsed, err := url.Parse(cfg.URL)
    if err != nil {
        return ""   // silently returns empty string on parse error
    }
    return parsed.Query().Get("sslmode")
}
```

If the user makes a typo in the database URL (e.g., `postresql://...`), `SSLMode()` returns `""`, which means `sslModeRequiresServerVerification` returns `false`, and the validation skips the CA cert check. A malformed URL could accidentally bypass a required cert validation.

---

## 2. HTTP/API Layer

### 2.1 Runner Webhook API Is Under-Documented for Producers

The README lists:
- `GET /healthz`
- `POST /ingest/<mapping_id>`

But it **does not document the request payload format**. A user setting up a CockroachDB changefeed must know that the runner expects:
- CockroachDB CDC `enriched` envelope with `source` metadata
- `payload` array with `length` field
- Row events with `op`, `key`, `after` (for upsert) or just `op` + `key` (for delete)
- `source.database_name`, `source.schema_name`, `source.table_name`
- Or alternatively a `resolved` message

The only documentation for this is in `crates/runner/src/webhook_runtime/payload.rs`, which is source code, not user docs. A new user cannot construct a valid webhook payload without reading Rust code.

### 2.2 Verify API Uses Flat Filter Fields (Good), But the Schema Is Implicit

```json
{
  "include_schema": "^public$",
  "include_table": "^(accounts|orders)$",
  "exclude_schema": "audit",
  "exclude_table": "^tmp_"
}
```

These are flat string fields interpreted as regex. This is ergonomic for curl, but:
- There is no JSON schema or OpenAPI spec to discover them.
- Invalid regex produces a structured error, but only after the job is accepted and then fails during runner compilation.
- The fields accept raw regex, which is powerful but dangerous (e.g., `accounts;$(touch /tmp/pwned)` is accepted as a valid regex string, though it is a command injection attempt if later evaluated in a shell).

### 2.3 Verify Job Lifecycle Has No Persistence

```go
// cockroachdb_molt/molt/verifyservice/service.go:137-163
func (s *Service) finishJob(...) {
    // ...
    s.lastCompletedJob = job  // only keeps the most recent job
    s.activeJob = nil
}
```

If the verify service process restarts, all job history is lost. For a long-running verify service, this means:
- A user polling for a job ID after a rolling restart gets 404.
- There is no audit trail.
- The metrics counter resets.

The tests even verify this behavior:
```go
func TestJobResultsAreLostAfterProcessRestart(t *testing.T) { ... }
```

This is acknowledged but not documented in the README. A new user will be surprised when their `GET /jobs/job-000001` returns 404 after a pod restart.

### 2.4 Only One Concurrent Verify Job

```go
var errJobAlreadyRunning = newOperatorError("job_state", "job_already_running", "a verify job is already running")
```

This is a reasonable operational constraint, but the API returns `409 Conflict` with no hint about *when* the current job will finish or how to check its status. A new user might not know to `GET /jobs/job-000001` to see the active job.

### 2.5 Runner and Verify Have Different API Philosophies

| Aspect | Runner | Verify |
|--------|--------|--------|
| Pattern | Webhook receiver (push) | REST resource (pull/poll) |
| Auth | mTLS (implied by HTTPS + CockroachDB webhook) | None / mTLS at listener level |
| Job model | Stateless per-request | Stateful job lifecycle |
| Response format | Plain text (`ok`) or empty body | Rich JSON with structured errors |

This is not necessarily wrong, but it means a user must context-switch between two API styles when operating the full pipeline.

### 2.6 Metrics Endpoints Use Different Formats

- Runner: `text/plain; version=0.0.4; charset=utf-8` (Prometheus text format)
- Verify: Prometheus text format via `newMetricsHandler(s)`

At least these are consistent, but the runner hardcodes the content type string:
```rust
[(
    axum::http::header::CONTENT_TYPE,
    "text/plain; version=0.0.4; charset=utf-8",
)],
runtime.metrics().render(),
```

This is a minor papercut: if the Prometheus format version changes, this is a magic string to hunt down.

---

## 3. CLI Verbosity and Operator Surface

### 3.1 CLI Structure Is Clean But Minimal

```
setup-sql
  emit-cockroach-sql --config <path> [--format text|json]
  emit-postgres-grants --config <path> [--format text|json]

runner
  validate-config --config <path>
  run --config <path>

verify-service-image
  --config <path>
```

The tests enforce that this stays flat (`max_action_depth() == 1` or `0`). This is good for novice users.

However, the **verify service image has no subcommand**; it just takes `--config` and runs. This is inconsistent with `runner` and `setup-sql`, which both require an explicit action verb. A user might type `verify-service validate-config --config ...` out of habit and get an error.

### 3.2 Log Format Is Global but Behaves Differently Per Command

All three binaries support `--log-format json`, but:
- `setup-sql` and `runner` write JSON logs to **stderr** and command output to **stdout**.
- `verify-service` (Go) uses zerolog and writes JSON to stderr, but its HTTP responses are always JSON.

In text mode:
- `runner validate-config` prints `config valid: config=... mappings=... webhook=... tls=...` to **stdout**.
- `setup-sql emit-cockroach-sql` prints SQL to **stdout**.

This stdout/stderr split is documented and tested, but it is a subtle contract that new users must internalize.

### 3.3 No Dry-Run or Preview Mode for Runner

`runner validate-config` checks syntax and file existence, but it does **not** verify that:
- The PostgreSQL destination is reachable.
- The tables exist.
- The credentials work.
- The TLS certificates are valid for the hostname.

A `validate-config --deep` or `run --dry-run` mode would let users catch connectivity issues before starting the long-lived runtime.

The `setup-sql` command does not validate that the CockroachDB URL is reachable either. It only validates that the URL string is present and the CA cert file exists.

---

## 4. Onboarding Friction (Walking Through the README)

### 4.1 The README Requires 7 Files Before First Run

From the README quick start, a user must create:
1. `config/cockroach-setup.yml`
2. `config/postgres-grants.yml`
3. `config/runner.yml`
4. `config/verify-service.yml`
5. `setup-sql.compose.yml` (optional)
6. `runner.compose.yml` (optional)
7. `verify.compose.yml` (optional)

Plus 5+ certificate files:
- `config/certs/server.crt`
- `config/certs/server.key`
- `config/certs/destination-ca.crt`
- `config/certs/destination-client.crt`
- `config/certs/destination-client.key`
- `config/certs/source-ca.crt`
- `config/certs/source-client.crt`
- `config/certs/source-client.key`
- `config/certs/client-ca.crt`

That is a lot of filesystem setup before anything runs. The README does a good job of providing copy-pasteable snippets, but the sheer volume creates opportunities for mismatch.

### 4.2 The `cockroach-bootstrap.sql` Has a Manual Substitution Step

```sql
-- Replace __CHANGEFEED_CURSOR__ below with the decimal cursor returned above
CREATE CHANGEFEED FOR TABLE ... INTO 'webhook-https://...' WITH cursor = '__CHANGEFEED_CURSOR__', ...
```

The user must:
1. Run the emitted SQL to get the cursor.
2. Copy the cursor value.
3. Edit the SQL file to substitute the placeholder.
4. Run the edited SQL.

This is documented, but it breaks the "one command and done" ideal. A more ergonomic flow might emit a shell script that captures the cursor and creates the changefeed in one step, or provide a `--cursor auto` mode.

### 4.3 No Single Command to Validate the Whole Pipeline

A user can validate each config in isolation:
- `setup-sql emit-cockroach-sql --config ...` (implicitly validates)
- `runner validate-config --config ...`

But there is no command that checks:
- Do the mapping IDs match across cockroach-setup and runner configs?
- Do the table lists match?
- Is the webhook base URL in the cockroach config pointing to the runner's bind address?
- Are the TLS certificates consistent between what CockroachDB expects and what the runner serves?

These cross-config validations must be done manually or not at all.

---

## 5. Recommendations

### High Priority

1. **Unify configs or provide a merge command.** A single `migration.yml` with sections for `source`, `runner`, `grants`, and `verify` would eliminate duplication. If unification is impossible, add a `validate-pipeline --setup-config ... --runner-config ...` command that cross-checks mapping IDs and table lists.

2. **Add env var substitution to runner config.** Support `${RUNNER_PASSWORD_APP_A}` or similar so the YAML can be checked into Git. Alternatively, support a `--secrets-file` overlay.

3. **Support connection strings for runner destination.** Allow `url: postgresql://...` as an alternative to decomposed fields. Most PostgreSQL users already have a working connection string.

4. **Document the webhook payload format in the README.** Provide a complete example JSON body for `POST /ingest/<mapping_id>` so users do not need to read `payload.rs`.

5. **Add HTTP mode to runner webhook.** A `webhook.mode: http` option (defaulting to `https` in production) would remove the TLS certificate generation step from local development.

### Medium Priority

6. **Add a `--deep-validate` or `--dry-run` mode to runner.** Connect to the destination PostgreSQL, verify tables exist, and report any mismatches before starting the runtime.

7. **Persist verify job results.** Write job state to a local SQLite file or allow a PostgreSQL connection for persistence. At minimum, document the amnesia behavior prominently.

8. **Make verify-service CLI consistent.** Add an explicit `run` subcommand so the surface matches `runner run --config ...`.

9. **Improve error messages with context.** Instead of `entries must use schema.table`, say `found "customers", expected "schema.table" format like "public.customers"`.

10. **Add a `validate-cockroach-url` step to setup-sql.** Check that the CockroachDB URL is reachable before emitting SQL, or at least parse it more strictly.

### Low Priority

11. **Consider OpenAPI or JSON Schema for verify API.** This would make client generation and discovery easier.

12. **Automate the cursor substitution.** Provide a mode where `setup-sql` emits a shell script that runs `SELECT cluster_logical_timestamp()` and pipes it into `CREATE CHANGEFEED`, or emit two files: one for setup, one for the changefeed creation.

13. **Standardize TLS config patterns.** Pick one approach (YAML enum + paths vs URL query params + paths) and use it consistently across runner and verify.

---

## 6. Conclusion

The project has a strong foundation: strict validation, tested contracts, a focused README, and clear separation of concerns. However, the **new user experience is burdened by config fragmentation, secrets-in-YAML, mandatory TLS for local dev, and missing cross-component validation**. The most impactful fixes would be: (1) unified or cross-validated config, (2) env var support for secrets, and (3) HTTP mode for the runner webhook. These three changes would reduce the time-to-first-migration from "hours of file wrangling" to "minutes of copy-paste."
