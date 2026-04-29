## Task: Verify multi-db config <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Replace the current single source/destination verify-service database config with a structured multi-database config that can share default connection credentials across many database mappings, allow per-database overrides where needed, and also support configurations where every database mapping supplies complete connection settings. The higher order goal is to make the verify container ergonomic for real operators running one verify service against multiple CockroachDB-to-PostgreSQL database pairs, without forcing repeated credentials or raw connection URLs everywhere.

Current product decision from the operator config discussion:
- the verify-service config must support multiple database mappings
- do not support a bare scalar database list such as `- app`; every database entry must be an object with fields
- do not use generic pass-through `params` or `application_name` in the baseline schema
- prefer structured connection fields over raw connection URLs for the operator-facing config
- common source and destination connection settings may be supplied once at `verify.source` and `verify.destination`
- each `verify.databases[]` entry may specify only database names when defaults provide connection settings
- each `verify.databases[]` entry may override source and/or destination connection settings such as `user` and `password_file`
- the config must also support no default source/destination credentials, where each database entry supplies complete source/destination connection settings
- config validation failures must produce clear operator errors in logs; errors must not be swallowed or hidden

The current verify-service config shape is single-pair oriented and lives primarily in:
- `cockroachdb_molt/molt/verifyservice/config.go`
- `cockroachdb_molt/molt/verifyservice/verify_runner.go`
- `cockroachdb_molt/molt/verifyservice/raw_table.go`
- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
- `docs/operator-guide/config-reference.md`
- `docs/operator-guide/verify-service.md`
- `openapi/verify-service.yaml`

The new implementation should resolve each configured database entry into an internal source/destination connection pair before invoking MOLT verify. The MOLT verify hot path should still receive exactly two ordered connections per verify invocation. If a job targets all databases in the future, the implementation should run pairs sequentially unless a later task explicitly introduces concurrent execution and database-aware aggregation.

Required config examples:

```yaml
# 1. Default credentials; each database only specifies database names.
listener:
  bind_addr: 0.0.0.0:8080
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt

verify:
  raw_table_output: false

  source:
    host: source.internal
    port: 26257
    user: verify_source
    password_file: /config/secrets/source-password
    sslmode: verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key

  destination:
    host: destination.internal
    port: 5432
    user: verify_target
    password_file: /config/secrets/destination-password
    sslmode: verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt

  databases:
    - name: app
      source_database: app
      destination_database: app

    - name: billing
      source_database: billing
      destination_database: billing

    - name: support
      source_database: support
      destination_database: support_archive
```

```yaml
# 2. Default credentials; one database overrides user/password.
listener:
  bind_addr: 0.0.0.0:8080
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt

verify:
  raw_table_output: false

  source:
    host: source.internal
    port: 26257
    user: verify_source
    password_file: /config/secrets/source-password
    sslmode: verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key

  destination:
    host: destination.internal
    port: 5432
    user: verify_target
    password_file: /config/secrets/destination-password
    sslmode: verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt

  databases:
    - name: app
      source_database: app
      destination_database: app

    - name: billing
      source_database: billing
      destination_database: billing

    - name: audit
      source_database: audit
      destination_database: audit
      source:
        user: verify_audit_source
        password_file: /config/secrets/audit-source-password
      destination:
        user: verify_audit_target
        password_file: /config/secrets/audit-destination-password
```

```yaml
# 3. No default credentials; each database supplies full source/destination settings.
listener:
  bind_addr: 0.0.0.0:8080
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt

verify:
  raw_table_output: false

  databases:
    - name: app
      source:
        host: source.internal
        port: 26257
        database: app
        user: verify_app_source
        password_file: /config/secrets/app-source-password
        sslmode: verify-full
        tls:
          ca_cert_path: /config/certs/source-ca.crt
          client_cert_path: /config/certs/source-client.crt
          client_key_path: /config/certs/source-client.key
      destination:
        host: destination.internal
        port: 5432
        database: app
        user: verify_app_target
        password_file: /config/secrets/app-destination-password
        sslmode: verify-ca
        tls:
          ca_cert_path: /config/certs/destination-ca.crt

    - name: billing
      source:
        host: source.internal
        port: 26257
        database: billing
        user: verify_billing_source
        password_file: /config/secrets/billing-source-password
        sslmode: verify-full
        tls:
          ca_cert_path: /config/certs/source-ca.crt
          client_cert_path: /config/certs/source-client.crt
          client_key_path: /config/certs/source-client.key
      destination:
        host: destination.internal
        port: 5432
        database: billing_prod
        user: verify_billing_target
        password_file: /config/secrets/billing-destination-password
        sslmode: verify-ca
        tls:
          ca_cert_path: /config/certs/destination-ca.crt
```

In scope:
- define the new structured verify-service YAML schema
- remove the single top-level `verify.source.url` / `verify.destination.url` operator-facing shape rather than preserving legacy compatibility
- build PostgreSQL/CockroachDB connection strings internally from structured fields using safe URL construction, not ad hoc string concatenation
- support default source/destination endpoint settings at `verify.source` and `verify.destination`
- support per-database source/destination overrides
- support fully specified per-database source/destination settings when defaults are absent
- reject scalar database entries; `verify.databases[]` entries must be mapping objects
- reject generic pass-through params and unsupported fields through known-field YAML decoding
- update CLI validation output and structured JSON logs so invalid configs identify the precise field and reason where practical
- update operator docs and examples to use the three supported shapes above
- update raw-table handling if needed so multiple configured databases cannot be ambiguous

Out of scope:
- preserving the old single-pair URL config shape
- supporting scalar database entries
- supporting generic connection parameter pass-through
- adding concurrent multi-database verify execution
- changing MOLT verify comparison semantics
- adding a scheduling system for recurring verify jobs

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers parsing, validation, and resolved connection construction for the default-credentials config example above
- [ ] Red/green TDD covers parsing, validation, inheritance, and resolved connection construction for the default-credentials-with-per-database-override config example above
- [ ] Red/green TDD covers parsing, validation, and resolved connection construction for the no-default-credentials config example above
- [ ] Red/green TDD proves scalar database entries are rejected with a clear field-specific error
- [ ] Red/green TDD proves missing required inherited fields are rejected with a clear field-specific error, for example a database entry without defaults and without full source/destination settings
- [ ] Red/green TDD proves duplicate database names are rejected with a clear error naming the duplicate
- [ ] Red/green TDD proves invalid TLS combinations are rejected after defaults and overrides are merged, including missing CA for `verify-ca` / `verify-full` and client cert without client key
- [ ] Red/green TDD proves unsupported fields such as `params` or `application_name` are rejected by config loading
- [ ] Red/green TDD proves `validate-config --log-format json` logs clear operator errors for wrong config values, including category, code, message, and details that identify the bad field and reason
- [ ] Red/green TDD proves verify execution resolves the requested configured database to the expected source/destination connection strings before invoking the MOLT verify runner
- [ ] Red/green TDD covers raw table requests when multiple databases are configured so the API either requires an explicit database selector or rejects ambiguity with a clear error
- [ ] Operator docs and config reference include the three full examples from this task and remove old single-URL examples
- [ ] `make check` - passes cleanly
- [ ] `make test` - passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` - passes cleanly
- [ ] If this task impacts ultra-long tests or their selection: `make test-long` - passes cleanly (ultra-long-only)
</acceptance_criteria>
