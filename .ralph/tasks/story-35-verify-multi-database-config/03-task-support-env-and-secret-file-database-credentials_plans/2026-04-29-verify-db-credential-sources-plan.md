# Plan: Verify-Service Credential Sources

## References

- Task:
  - `.ralph/tasks/story-35-verify-multi-database-config/03-task-support-env-and-secret-file-database-credentials.md`
- Existing verify-service config and resolution seams:
  - `cockroachdb_molt/molt/verifyservice/config.go`
  - `cockroachdb_molt/molt/verifyservice/resolved_config.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner.go`
  - `cockroachdb_molt/molt/verifyservice/raw_table.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
- Tests to extend through public interfaces:
  - `cockroachdb_molt/molt/verifyservice/config_test.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner_test.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
  - `cockroachdb_molt/molt/verifyservice/runtime_test.go`
- Operator docs and examples to update:
  - `docs/operator-guide/config-reference.md`
  - `docs/operator-guide/verify-service.md`
  - verify-service `testdata/*.yml`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown already fixes the operator-facing schema for this turn.
- This turn is planning-only because task 03 had no existing `<plan>` entry when opened.
- No backwards compatibility is allowed.
  - Remove `user` and `password_file` from the operator-facing verify-service config.
  - Reject `password_file` everywhere known-field decoding applies instead of silently translating it.
- The HTTP jobs schema from task 02 stays intact.
  - This task changes config loading, resolution, runtime connection construction, CLI validation output, examples, and docs.
- The runtime must continue to fail closed.
  - Missing env vars, unreadable secret files, empty resolved credentials, and malformed credential objects are validation failures.
- If implementation proves that startup logging of source type and field path needs a different operator-error shape than this plan anticipates, switch the plan back to `TO BE VERIFIED` and stop immediately.

## Current State Summary

- `DatabaseConfig` still mixes operator-facing config input with partially resolved connection concerns.
  - It exposes `user` and `password_file` directly.
- `ResolvedConnection` is not actually fully resolved.
  - It still carries `PasswordFile`, and `ConnectionString()` converts that to `passfile=` instead of embedding a resolved password.
- Default merging and validation happen in the same path as runtime connection preparation.
  - That makes it hard to express “merge config first, then resolve secrets once, then build safe URLs.”
- Tests and examples still assume the legacy credential contract everywhere:
  - YAML fixtures use `user` and `password_file`.
  - config tests assert `passfile=` in connection strings.
  - docs still describe `password_file` as the supported file secret path.

## Boundary Problem To Flatten

- The main boundary smell is that credential indirection currently lives in the wrong layer.
  - Operator config fields, merge rules, secret source interpretation, and connection-string construction are coupled through `DatabaseConfig` and `ResolvedConnection`.
- Execution should flatten this into two clear stages:
  - merged operator config with unresolved credential source declarations
  - fully resolved internal connection data with concrete username/password strings
- This is the `improve-code-boundaries` target for the task.
  - Introduce one reusable credential value type shared by `username` and `password`.
  - Resolve credential sources in one dedicated helper instead of letting `ConnectionString()` inspect config-like fields.
  - Delete the `password_file`-to-`passfile` path entirely rather than carrying both models.

## Public Contract To Establish

- Every verify-service database credential field uses the same schema:
  - scalar string shorthand
  - or object with exactly one of:
    - `value`
    - `env_ref`
    - `secret_file`
- This applies in all source and destination database config positions:
  - top-level defaults
  - per-database overrides
  - fully specified per-database blocks
- `username: literal` and `password: literal` mean the same thing as `{ value: literal }`.
- `secret_file` resolution trims exactly one trailing newline and preserves interior whitespace.
- Validation errors must identify:
  - the failing field path
  - the failing source type when relevant
- Validation errors and logs must never include resolved secret values.
- Connection strings must be built from resolved username/password values using safe URL construction.
  - Credentials should appear only in the URL userinfo sent to the DB client, never in validation logs.

## Proposed Type Shape

- Replace the legacy credential fields on `DatabaseConfig`:
  - `Username CredentialValue `yaml:"username,omitempty"``
  - `Password CredentialValue `yaml:"password,omitempty"``
- Introduce one reusable operator-facing credential type:
  - `CredentialValue`
    - custom YAML unmarshalling to accept either scalar string or object
    - fields:
      - `Value string`
      - `EnvRef string`
      - `SecretFile string`
- Introduce one internal resolved credential type:
  - `ResolvedCredential`
    - concrete string value only
- Keep `ResolvedConnection` fully internal and fully resolved:
  - `Username string`
  - `Password string`
  - remove `PasswordFile`
- Keep URL construction centralized in `ResolvedConnection.ConnectionString()`.
  - Use `url.UserPassword` when password is present.
  - Use `url.User` only if the final contract ever allows blank password, otherwise validation should reject blank password.

## Validation And Resolution Rules

- YAML decoding must continue to use known fields.
  - `password_file` should now fail as an unknown field.
- Merging order stays:
  - merge default source/destination config with per-database overrides
  - then resolve `username` and `password` on the effective config
- Credential object validation:
  - reject zero sources
  - reject more than one source
  - reject empty `env_ref`
  - reject empty `secret_file`
  - reject empty `value`
- Credential resolution:
  - `value` resolves directly
  - `env_ref` must exist and be non-empty
  - `secret_file` must be readable and non-empty after trimming one trailing newline
- Effective connection validation runs on resolved values:
  - host, port, database, username, password, sslmode, TLS requirements
- Operator errors should use field-specific paths such as:
  - `verify.source.username.env_ref`
  - `verify.databases[2].destination.password.secret_file`
  - `verify.databases[1].source.username`
- JSON validation logs should expose the path and source type in structured details without printing credential contents.

## TDD Slices

### Slice 1: Tracer Bullet For Scalar Credentials In Shared Defaults

- RED:
  - add a config-loading test using scalar `username` and `password` in top-level `verify.source` / `verify.destination`
  - assert the resolved source and destination connection strings embed the literal credentials safely
  - assert scalar form behaves like explicit `{ value: ... }`
- GREEN:
  - introduce `CredentialValue` YAML decoding
  - wire `username` / `password` into `DatabaseConfig`
  - remove legacy `user` / `password_file` fields from config types
- REFACTOR:
  - keep scalar-vs-object handling inside `CredentialValue`, not scattered across config resolution

### Slice 2: Default Env Ref Credentials Resolve Into Final Connection Strings

- RED:
  - add a config test that sets environment variables for default source and destination credentials
  - assert `ResolveDatabase()` and `ConnectionString()` use the env-sourced username/password
- GREEN:
  - implement env resolution on the effective merged config
- REFACTOR:
  - keep environment access behind one credential resolver helper so tests and runtime use the same path

### Slice 3: Default Secret File Credentials Resolve With Kubernetes-Style Newlines

- RED:
  - add a config test with temp secret files that end in one trailing newline
  - assert the resolved password trims only that final newline and still URL-encodes correctly
- GREEN:
  - implement secret-file resolution with the required newline normalization
- REFACTOR:
  - keep file reading and normalization in the credential resolver layer, not in `ConnectionString()`

### Slice 4: Per-Database Override Credentials Support Mixed Source Kinds

- RED:
  - add a config test where one database override mixes scalar, `value`, `env_ref`, and `secret_file` across source and destination
  - assert inherited non-credential fields remain intact while effective credentials come from the overrides
- GREEN:
  - make merge logic preserve credential declarations field-by-field before resolution
- REFACTOR:
  - avoid duplicate merge code for username and password by treating them as the same type

### Slice 5: No-Defaults Per-Database Config Accepts The Same Credential Schema

- RED:
  - add a config test for the fully specified per-database form with direct values, env refs, and secret files
  - assert each database resolves independently with the same credential behavior as the defaults path
- GREEN:
  - ensure the existing no-defaults resolution path reuses the same credential resolver
- REFACTOR:
  - confirm there is one shared “effective database config -> resolved connection” path

### Slice 6: Invalid Credential Sources Fail Closed With Field-Specific Errors

- RED:
  - add config tests proving failures for:
    - empty `env_ref`
    - unset env var
    - env var set to empty string
    - empty `secret_file`
    - unreadable secret file
    - empty file after newline normalization
    - zero-source object
    - multi-source object
  - assert error messages name the specific field path and source type
  - assert the error text does not contain the secret content
- GREEN:
  - implement strict credential validation and resolution errors
- REFACTOR:
  - keep error formatting centralized so runner/CLI/tests all see the same safe message shape

### Slice 7: Legacy `password_file` Is Rejected Everywhere Known-Field Decoding Applies

- RED:
  - update or add config tests using `password_file` under defaults and per-database blocks
  - assert YAML decoding rejects the unknown field
- GREEN:
  - rely on the new config struct shape and known-field decoding to reject it
- REFACTOR:
  - delete outdated fixtures that preserve the old field name

### Slice 8: JSON Validation Logs Surface Safe Structured Credential Failures

- RED:
  - extend `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - assert `validate-config --log-format json` includes category/code/details that identify the failing field and source type
  - assert stderr does not contain resolved usernames or passwords
- GREEN:
  - propagate the new operator error details from config loading through CLI logging
- REFACTOR:
  - keep secret-safe error shaping in the config/operator-error layer, not the cobra command

### Slice 9: Runtime Paths Use Fully Resolved Credentials Only

- RED:
  - extend runner and raw-table oriented tests only where needed to prove connections now use password-bearing URLs rather than `passfile=`
  - assert runtime paths still route through resolved database pairs and do not rebuild credential logic independently
- GREEN:
  - switch runtime consumers to the fully resolved `ResolvedConnection`
- REFACTOR:
  - remove any remaining legacy passfile assumptions from runtime tests or helpers

### Slice 10: Docs, Fixtures, And Examples Move To The Canonical Credential Schema

- RED:
  - no brittle string-matching unit tests
  - rely on project checks plus updated fixture-based config tests to verify the documented schema remains executable
- GREEN:
  - update verify-service docs and examples to use `username` / `password`
  - include one default-credentials example and one no-defaults per-database example showing `value`, `env_ref`, and `secret_file`
- REFACTOR:
  - remove obsolete `user` / `password_file` references from verify-service docs and fixtures instead of keeping parallel examples

## Execution Order

- Execute slices strictly in vertical red/green order.
- Start with config parsing and resolution because CLI logging, runner execution, raw-table reads, docs, and fixtures all depend on that boundary.
- Do not touch unrelated story-35 HTTP job behavior unless a failing test proves the resolved connection contract changed a shared assertion.
- After all slices are green, do one explicit `improve-code-boundaries` cleanup pass to verify:
  - `DatabaseConfig` no longer mixes unresolved secret sources with resolved runtime connection state
  - `ResolvedConnection` contains only concrete values needed by runtime
  - no `password_file` compatibility code or doc references remain in the verify-service path

## Verification Gates

- Required before marking the task done:
  - `make check`
  - `make lint`
  - `make test`
- Do not run `make test-long` unless execution proves this task changed the ultra-long lane selection.
- Do not skip failing tests or lints.
  - Fix the code or stop and surface the blocker.

## Switch-Back Conditions

- Switch this plan back to `TO BE VERIFIED` immediately if:
  - the scalar-or-object YAML contract for `CredentialValue` cannot be expressed cleanly without changing task-approved config shape
  - startup log requirements need a materially different operator-error contract than the current CLI/logging path can safely expose
  - runtime consumers need unresolved secret references instead of concrete credentials, which would invalidate the boundary cleanup above
  - the repository reveals a stronger shared credential type outside verify-service that should replace this design

Plan path: `.ralph/tasks/story-35-verify-multi-database-config/03-task-support-env-and-secret-file-database-credentials_plans/2026-04-29-verify-db-credential-sources-plan.md`

NOW EXECUTE
