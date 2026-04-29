## Task: Support env and secret-file database credentials in the multi-db config <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Extend the new structured verify-service multi-database config so every database credential field can be resolved from either an inline value, an environment variable reference, or a secret file. The higher order goal is to let operators run the verify service with Kubernetes/container secret injection patterns without putting database usernames or passwords directly in the YAML, while keeping source and destination database config on one shared credential schema everywhere credentials can appear.

This task belongs to story 35 and builds on task 1's new multi-database config model. Task 1 introduces default source/destination database settings at `verify.source` and `verify.destination`, per-database `source`/`destination` overrides, and fully specified per-database source/destination settings. This task adds credential indirection to that same schema rather than creating a separate credential model.

Required product decision:
- both database credential fields, `username` and `password`, must use the same credential value schema
- the credential value schema must support either a direct scalar string or exactly one object source:
  - `username: literal_value` means the same thing as `username: { value: literal_value }`
  - `password: literal_value` means the same thing as `password: { value: literal_value }`
  - `value: literal_value`
  - `env_ref: ENVIRONMENT_VAR_NAME`
  - `secret_file: /path/to/secret.txt`
- this support must work for source database config, destination database config, default database config, per-database overrides, and fully specified per-database config
- do not preserve a separate legacy `password_file` field; the supported file-based credential source is `secret_file`
- do not add different field names such as `username_env`, `password_env`, `username_file`, or `password_file`
- do not silently continue if an environment variable is unset, empty, or if a secret file cannot be read
- config validation and startup logs must report the failing field and source type, but must not print resolved secret values

End-goal config shape:

```yaml
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
    username:
      env_ref: VERIFY_SOURCE_USERNAME
    password:
      secret_file: /config/secrets/source-password
    sslmode: verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key

  destination:
    host: destination.internal
    port: 5432
    username: verify_target
    password:
      env_ref: VERIFY_DESTINATION_PASSWORD
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
        username:
          value: verify_audit_source
        password:
          env_ref: AUDIT_SOURCE_PASSWORD
      destination:
        username:
          secret_file: /config/secrets/audit-destination-username
        password:
          secret_file: /config/secrets/audit-destination-password
```

The no-defaults form must use the exact same credential field shape inside every per-database source/destination object:

```yaml
listener:
  bind_addr: 0.0.0.0:8080

verify:
  raw_table_output: false

  databases:
    - name: app
      source:
        host: source.internal
        port: 26257
        database: app
        username:
          env_ref: APP_SOURCE_USERNAME
        password:
          secret_file: /config/secrets/app-source-password
        sslmode: verify-full
        tls:
          ca_cert_path: /config/certs/source-ca.crt
      destination:
        host: destination.internal
        port: 5432
        database: app
        username: verify_app_target
        password:
          env_ref: APP_DESTINATION_PASSWORD
        sslmode: verify-ca
        tls:
          ca_cert_path: /config/certs/destination-ca.crt
```

Resolution rules:
- resolve defaults and per-database overrides first, then resolve credential values for the final effective source/destination database config
- scalar `username` and scalar `password` values are interpreted as direct literal values, exactly like the explicit `value` field
- `env_ref` must name an environment variable; the resolved value is the environment variable's value
- `secret_file` must read the file content as the credential value; trim one trailing newline if present so Kubernetes/Docker secret files work naturally
- if a secret file contains additional interior whitespace, preserve it; only the final line ending should be normalized
- fail validation if a credential object contains zero sources or more than one source
- fail validation if `env_ref` is empty, references an unset variable, or resolves to an empty string
- fail validation if `secret_file` is empty, the file cannot be read, or the file resolves to an empty credential after newline normalization
- fail validation if the old `password_file` field appears anywhere in the new config
- build CockroachDB/PostgreSQL connection strings from resolved credential values using safe URL construction, without printing the raw password in logs or validation JSON

Likely files to update:
- `cockroachdb_molt/molt/verifyservice/config.go`
- `cockroachdb_molt/molt/verifyservice/config_test.go`
- `cockroachdb_molt/molt/verifyservice/verify_runner.go`
- `cockroachdb_molt/molt/verifyservice/raw_table.go` if raw-table connection resolution shares the same database config path
- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
- `docs/operator-guide/config-reference.md`
- `docs/operator-guide/verify-service.md`
- any story-35 fixtures or examples introduced by task 1
- `openapi/verify-service.yaml` only if it documents config examples or error shape affected by this change

In scope:
- introduce one reusable credential value type for `username` and `password`
- wire that credential type into all source/destination database config locations
- update all task-1 examples and docs from `user`/`password_file` to `username`/`password` with the new credential object shape
- ensure per-database override merging works before credential resolution
- ensure validation errors are field-specific and do not leak secret values
- ensure generated connection strings use resolved credentials and remain safely escaped

Out of scope:
- adding secret providers other than environment variables and local files
- adding hot reload for changed environment variables or secret files
- adding encryption/decryption support
- adding backward compatibility for `password_file`, raw URL credentials, or old single-pair config
- changing MOLT verify comparison behavior
- changing the HTTP job request schema from task 2

</description>


<acceptance_criteria>
- [ ] Red/green TDD proves direct scalar `username` and direct scalar `password` values are accepted and interpreted exactly like explicit `value` credentials
- [ ] Red/green TDD proves `username.env_ref` and `password.env_ref` resolve for default source/destination database config and are used in the constructed source/destination connection strings
- [ ] Red/green TDD proves `username.secret_file` and `password.secret_file` resolve for default source/destination database config and are used in the constructed source/destination connection strings
- [ ] Red/green TDD proves per-database source/destination credential overrides can independently use direct scalar values, `value`, `env_ref`, and `secret_file`
- [ ] Red/green TDD proves the no-defaults per-database config form accepts the same `username`/`password` credential object schema for every source and destination database
- [ ] Red/green TDD proves missing, unset, and empty `env_ref` values fail with clear field-specific errors that do not include secret values
- [ ] Red/green TDD proves missing, unreadable, and empty `secret_file` values fail with clear field-specific errors that do not include secret values
- [ ] Red/green TDD proves credential objects with zero sources or multiple sources are rejected with clear field-specific errors
- [ ] Red/green TDD proves the obsolete `password_file` field is rejected everywhere known-field YAML decoding applies
- [ ] Red/green TDD proves secret files trim one trailing newline while preserving intentional interior credential content
- [ ] Red/green TDD proves validation logs and `validate-config --log-format json` identify the failing field and source type without printing resolved username/password values
- [ ] Operator docs and config examples show `username` and `password` using direct scalar values, `value`, `env_ref`, and `secret_file`, including one default-credentials example and one no-defaults per-database example
- [ ] `make check` - passes cleanly
- [ ] `make test` - passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` - passes cleanly
- [ ] If this task impacts ultra-long tests or their selection: `make test-long` - passes cleanly (ultra-long-only)
</acceptance_criteria>
