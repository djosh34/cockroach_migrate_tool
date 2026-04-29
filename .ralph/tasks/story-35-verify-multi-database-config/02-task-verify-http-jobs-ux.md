## Task: Verify HTTP jobs UX <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Redesign the verify-service jobs HTTP UX around the new multi-database config model. The higher order goal is to make the verify-service HTTP API pleasant and predictable for operators: requests should use simple glob matching, should not repeat connection details already present in config, and responses should show the actual matched databases, schemas, tables, progress, errors, and findings without forcing operators to make extra detail requests.

Required new features:
- ability to have multiple jobs running in memory at the same time, using a thread-safe in-memory job map or equivalent safe implementation
- ability to start one job that verifies all configured databases without the HTTP caller specifying each database name
- ability to list current and retained jobs with the same job JSON schema used by `GET /jobs/{job_id}`
- every job response must show which configured database names are included in the job
- general HTTP UX improvements for verify-service job creation, listing, polling, stopping, failures, and findings
- no new config knobs such as max jobs, job limits, or similar settings should be added as part of this task

Core API design decisions:
- use globs, not regexes
- request fields must not require source/destination hosts, credentials, TLS paths, database URLs, or other connection details already supplied in config
- do not use separate `all_databases` vs `databases` request modes; database matching is expressed through `databases`, and `["*"]` means all configured databases
- do not return a `mode` field
- do not return both `database` and `databases`; use one `databases` list
- do not return a top-level job `error`; database-specific errors live under the corresponding database entry
- top-level job `status` is derived from the per-database statuses
- do not return arbitrary labels; no `label` field
- do not return `current_table` unless the verify engine later emits structured table progress; this task should not parse free-text logs to invent a current table
- include `rows_checked` per database when known
- `GET /jobs` and `GET /jobs/{job_id}` must use the exact same job object schema; `GET /jobs` only wraps those job objects in an array
- if the request uses globs such as `*`, the job response must show the matched reality: concrete configured databases, concrete matched schemas, and concrete matched tables where known, not the requested glob strings
- `schemas` and `tables` may be `null` while unknown, then become concrete matched names once discovered
- `findings` may be `null` when no findings are present; failed/mismatched databases should include findings in that database entry

Top-level job status derivation:
- if any database is `running`, job is `running`
- if any database is `stopping`, job is `stopping`
- if all databases are `succeeded`, job is `succeeded`
- if one or more databases are `failed`, job is `failed`
- if all unfinished databases were stopped, job is `stopped`

No second top-level error. Request validation errors are not job objects; they stay as normal error responses.

Remove `label`.

`label` was considered as an optional human note, but it is not necessary and it adds one more field to support, store, test, and document. Drop it.

About `current_table`.

Do not include `current_table` unless the verify engine emits structured table progress. If the only thing available is free text like `"verifying public.invoices"`, parsing that is brittle and not worth turning into API contract. So for now: drop `current_table`.

Rows checked.

Per database row counts are useful. Include:

```json
"rows_checked": 123456
```

If not known yet:

```json
"rows_checked": null
```

Do not add counts like number of databases, number of processed tables, etc. Just row count per database.

Revised POST shape:
- global defaults:
  - `default_schema_match`
  - `default_table_match`
- `databases` can be mixed:
  - string shorthand
  - object with per-database overrides
- every database object must include `database_match`; do not allow object entries that only specify schema/table matching
- matches use globs
- `schema_match` and `table_match` can each be either string or array

Example verifying everything with defaults:

```bash
curl -sS -X POST "$VERIFY_API/jobs" \
  -H 'Content-Type: application/json' \
  -d '{}' | jq
```

Equivalent expanded meaning:

```json
{
  "default_schema_match": "*",
  "default_table_match": "*",
  "databases": ["*"]
}
```

Equivalent compact form:

```json
{
  "databases": "*"
}
```

Example with one database:

```bash
curl -sS -X POST "$VERIFY_API/jobs" \
  -H 'Content-Type: application/json' \
  -d '{
    "databases": ["billing"]
  }' | jq
```

Example with one detailed database object:

```bash
curl -sS -X POST "$VERIFY_API/jobs" \
  -H 'Content-Type: application/json' \
  -d '{
    "databases": {
      "database_match": "customer-*",
      "schema_match": ["public", "archive"],
      "table_match": ["orders*", "payments*"]
    }
  }' | jq
```

Example with global defaults:

```bash
curl -sS -X POST "$VERIFY_API/jobs" \
  -H 'Content-Type: application/json' \
  -d '{
    "default_schema_match": "public",
    "default_table_match": "*",
    "databases": ["app", "billing"]
  }' | jq
```

Example with mixed string and object entries:

```bash
curl -sS -X POST "$VERIFY_API/jobs" \
  -H 'Content-Type: application/json' \
  -d '{
    "default_schema_match": "public",
    "default_table_match": "*",
    "databases": [
      "app",
      {
        "database_match": "billing",
        "table_match": ["invoices", "payments"]
      },
      {
        "database_match": "support",
        "schema_match": ["public", "archive"],
        "table_match": "tickets*"
      },
      {
        "database_match": "customer-*",
        "schema_match": "*",
        "table_match": ["orders*", "payments*"]
      }
    ]
  }' | jq
```

Use `database_match` in object entries rather than `name`, because it can be a glob too. That keeps the mental model consistent:

```json
"databases": [
  "billing",
  {
    "database_match": "customer-*",
    "schema_match": "public",
    "table_match": "orders*"
  }
]
```

String entry:

```json
"billing"
```

means:

```json
{
  "database_match": "billing"
}
```

Object entry:

```json
{
  "database_match": "billing",
  "schema_match": "public",
  "table_match": ["invoices", "payments"]
}
```

means use custom table/schema matching for that database match.

Defaults:

```json
{
  "default_schema_match": "public",
  "default_table_match": "*",
  "databases": ["billing"]
}
```

means:

```json
{
  "databases": [
    {
      "database_match": "billing",
      "schema_match": "public",
      "table_match": "*"
    }
  ]
}
```

Request schema:

```json
{
  "default_schema_match": "public",
  "default_table_match": "*",
  "databases": [
    "app",
    {
      "database_match": "billing",
      "schema_match": "public",
      "table_match": ["invoices", "payments"]
    }
  ]
}
```

Rules:
- `default_schema_match` optional, defaults to `"*"`
- `default_table_match` optional, defaults to `"*"`
- `default_schema_match` can be string or array of strings
- `default_table_match` can be string or array of strings
- `databases` optional, defaults to `"*"`
- `databases` can be a string, object, or array
- when `databases` is a string, it is a database glob
- when `databases` is an object, it must have `database_match`
- when `databases` is an object, it is one detailed database match object
- when `databases` is an array, each string item is a database glob
- when `databases` is an array, each object item must have `database_match`
- object forms without `database_match` are invalid even if they contain `schema_match` or `table_match`
- `schema_match` optional, defaults to `default_schema_match`
- `table_match` optional, defaults to `default_table_match`
- `schema_match` can be string or array of strings
- `table_match` can be string or array of strings
- all match fields are globs, not regex
- normalize the flexible request shape once so the rest of the implementation does not care about string-vs-object or string-vs-array input forms

Matching precedence from most specific to least specific:
- object entry `schema_match` / `table_match` applies only to configured databases matched by that same object entry's `database_match`
- string database entries use `default_schema_match` and `default_table_match`
- top-level `databases` object entries use their own `schema_match` / `table_match` when present
- if `default_schema_match` is omitted, it behaves as `"*"`
- if `default_table_match` is omitted, it behaves as `"*"`
- if `databases` is omitted, it behaves as `"*"` and uses the default schema/table matches
- if multiple database entries match the same configured database, the service should merge the matched schema/table selections for that configured database rather than silently dropping one matching entry

Example with default matches as arrays:

```bash
curl -sS -X POST "$VERIFY_API/jobs" \
  -H 'Content-Type: application/json' \
  -d '{
    "default_schema_match": ["public", "archive"],
    "default_table_match": ["orders*", "payments*"],
    "databases": ["app", "billing"]
  }' | jq
```

POST response:

Request:

```bash
curl -sS -X POST "$VERIFY_API/jobs" \
  -H 'Content-Type: application/json' \
  -d '{
    "default_schema_match": "public",
    "default_table_match": "*",
    "databases": [
      "app",
      {
        "database_match": "billing",
        "table_match": ["invoices", "payments"]
      },
      {
        "database_match": "support",
        "schema_match": ["public", "archive"],
        "table_match": "tickets*"
      }
    ]
  }' | jq
```

Response:

```json
{
  "job_id": "job-000044",
  "status": "running",
  "created_at": "2026-04-29T14:25:33.009Z",
  "started_at": "2026-04-29T14:25:33.010Z",
  "finished_at": null,
  "databases": [
    {
      "name": "app",
      "status": "running",
      "started_at": "2026-04-29T14:25:33.010Z",
      "finished_at": null,
      "schemas": ["public"],
      "tables": null,
      "rows_checked": 0,
      "error": null,
      "findings": null
    },
    {
      "name": "billing",
      "status": "running",
      "started_at": "2026-04-29T14:25:33.010Z",
      "finished_at": null,
      "schemas": ["public"],
      "tables": null,
      "rows_checked": 0,
      "error": null,
      "findings": null
    },
    {
      "name": "support",
      "status": "running",
      "started_at": "2026-04-29T14:25:33.010Z",
      "finished_at": null,
      "schemas": ["public", "archive"],
      "tables": null,
      "rows_checked": 0,
      "error": null,
      "findings": null
    }
  ]
}
```

Why `tables: null` at start? Because the service may not have connected and resolved actual matched table names yet. Once discovery completes, `tables` becomes concrete matched table names.

GET one job while running:

```bash
curl -sS "$VERIFY_API/jobs/job-000044" | jq
```

```json
{
  "job_id": "job-000044",
  "status": "running",
  "created_at": "2026-04-29T14:25:33.009Z",
  "started_at": "2026-04-29T14:25:33.010Z",
  "finished_at": null,
  "databases": [
    {
      "name": "app",
      "status": "succeeded",
      "started_at": "2026-04-29T14:25:33.010Z",
      "finished_at": "2026-04-29T14:25:42.612Z",
      "schemas": ["public"],
      "tables": ["accounts", "orders"],
      "rows_checked": 24931,
      "error": null,
      "findings": null
    },
    {
      "name": "billing",
      "status": "running",
      "started_at": "2026-04-29T14:25:33.010Z",
      "finished_at": null,
      "schemas": ["public"],
      "tables": ["invoices", "payments"],
      "rows_checked": 18820,
      "error": null,
      "findings": null
    },
    {
      "name": "support",
      "status": "running",
      "started_at": "2026-04-29T14:25:33.010Z",
      "finished_at": null,
      "schemas": ["public", "archive"],
      "tables": ["tickets", "tickets_archive"],
      "rows_checked": 9021,
      "error": null,
      "findings": null
    }
  ]
}
```

Same schema. No top-level error. No `mode`. No `label`. No `current_table`.

GET all jobs:

```bash
curl -sS "$VERIFY_API/jobs" | jq
```

```json
[
  {
    "job_id": "job-000044",
    "status": "running",
    "created_at": "2026-04-29T14:25:33.009Z",
    "started_at": "2026-04-29T14:25:33.010Z",
    "finished_at": null,
    "databases": [
      {
        "name": "app",
        "status": "succeeded",
        "started_at": "2026-04-29T14:25:33.010Z",
        "finished_at": "2026-04-29T14:25:42.612Z",
        "schemas": ["public"],
        "tables": ["accounts", "orders"],
        "rows_checked": 24931,
        "error": null,
        "findings": null
      },
      {
        "name": "billing",
        "status": "running",
        "started_at": "2026-04-29T14:25:33.010Z",
        "finished_at": null,
        "schemas": ["public"],
        "tables": ["invoices", "payments"],
        "rows_checked": 18820,
        "error": null,
        "findings": null
      }
    ]
  },
  {
    "job_id": "job-000043",
    "status": "failed",
    "created_at": "2026-04-29T14:24:02.104Z",
    "started_at": "2026-04-29T14:24:02.105Z",
    "finished_at": "2026-04-29T14:24:19.883Z",
    "databases": [
      {
        "name": "app",
        "status": "succeeded",
        "started_at": "2026-04-29T14:24:02.105Z",
        "finished_at": "2026-04-29T14:24:11.902Z",
        "schemas": ["public"],
        "tables": ["accounts", "orders"],
        "rows_checked": 24931,
        "error": null,
        "findings": null
      },
      {
        "name": "billing",
        "status": "failed",
        "started_at": "2026-04-29T14:24:02.105Z",
        "finished_at": "2026-04-29T14:24:19.883Z",
        "schemas": ["public"],
        "tables": ["invoices", "payments"],
        "rows_checked": 38102,
        "error": {
          "category": "verify_result",
          "code": "mismatches_found",
          "message": "verify found mismatches"
        },
        "findings": [
          {
            "schema": "public",
            "table": "invoices",
            "kind": "row_mismatch",
            "primary_key": {
              "invoice_id": "inv_1042"
            },
            "message": "row differs between source and destination"
          },
          {
            "schema": "public",
            "table": "payments",
            "kind": "missing_in_destination",
            "primary_key": {
              "payment_id": "pay_902"
            },
            "message": "row exists in source but not destination"
          }
        ]
      }
    ]
  }
]
```

Same job schema, just wrapped in an array.

Failed connection:

```json
{
  "job_id": "job-000045",
  "status": "failed",
  "created_at": "2026-04-29T14:31:00.012Z",
  "started_at": "2026-04-29T14:31:00.013Z",
  "finished_at": "2026-04-29T14:31:01.424Z",
  "databases": [
    {
      "name": "billing",
      "status": "failed",
      "started_at": "2026-04-29T14:31:00.013Z",
      "finished_at": "2026-04-29T14:31:01.424Z",
      "schemas": null,
      "tables": null,
      "rows_checked": null,
      "error": {
        "category": "source_access",
        "code": "connection_failed",
        "message": "source connection failed",
        "details": [
          {
            "reason": "password authentication failed for user verify_billing_source"
          }
        ]
      },
      "findings": null
    }
  ]
}
```

No top-level error. The failed database contains the failure.

Request validation error:

This is not a job object because no job was created.

```bash
curl -sS -X POST "$VERIFY_API/jobs" \
  -H 'Content-Type: application/json' \
  -d '{
    "databases": [
      {
        "schema_match": "public"
      }
    ]
  }' | jq
```

```json
{
  "error": {
    "category": "request_validation",
    "code": "missing_database_match",
    "message": "database object must include database_match",
    "details": [
      {
        "field": "databases[0].database_match",
        "reason": "required when a database entry is an object"
      }
    ]
  }
}
```

Invalid glob:

```bash
curl -sS -X POST "$VERIFY_API/jobs" \
  -H 'Content-Type: application/json' \
  -d '{
    "databases": ["["]
  }' | jq
```

```json
{
  "error": {
    "category": "request_validation",
    "code": "invalid_glob",
    "message": "request contains an invalid glob",
    "details": [
      {
        "field": "databases[0]",
        "value": "[",
        "reason": "unterminated character class"
      }
    ]
  }
}
```

No configured database matched:

```bash
curl -sS -X POST "$VERIFY_API/jobs" \
  -H 'Content-Type: application/json' \
  -d '{
    "databases": ["does-not-exist"]
  }' | jq
```

```json
{
  "error": {
    "category": "request_validation",
    "code": "no_database_match",
    "message": "no configured databases matched the request",
    "details": [
      {
        "field": "databases",
        "value": "does-not-exist",
        "reason": "configured databases are: app, billing, support"
      }
    ]
  }
}
```

Final request shape:

```json
{
  "default_schema_match": "*",
  "default_table_match": "*",
  "databases": [
    "app",
    {
      "database_match": "billing",
      "schema_match": "public",
      "table_match": ["invoices", "payments"]
    }
  ]
}
```

Final job shape:

```json
{
  "job_id": "job-000044",
  "status": "running",
  "created_at": "2026-04-29T14:25:33.009Z",
  "started_at": "2026-04-29T14:25:33.010Z",
  "finished_at": null,
  "databases": [
    {
      "name": "app",
      "status": "succeeded",
      "started_at": "2026-04-29T14:25:33.010Z",
      "finished_at": "2026-04-29T14:25:42.612Z",
      "schemas": ["public"],
      "tables": ["accounts", "orders"],
      "rows_checked": 24931,
      "error": null,
      "findings": null
    },
    {
      "name": "billing",
      "status": "running",
      "started_at": "2026-04-29T14:25:33.010Z",
      "finished_at": null,
      "schemas": ["public"],
      "tables": ["invoices", "payments"],
      "rows_checked": 18820,
      "error": null,
      "findings": null
    }
  ]
}
```

This is the version to implement:
- parse flexible request
- normalize it once
- resolve matching configured databases
- create one `job`
- create one `databaseJob` per matched configured database
- each `databaseJob` owns its own status, timestamps, rows checked, error, and findings
- top-level job status is computed from child statuses
- `GET /jobs` returns an array of job objects
- `GET /jobs/{id}` returns one job object

Relevant files and boundaries:
- `cockroachdb_molt/molt/verifyservice/service.go`
- `cockroachdb_molt/molt/verifyservice/job.go`
- `cockroachdb_molt/molt/verifyservice/filter.go`
- `cockroachdb_molt/molt/verifyservice/result.go`
- `cockroachdb_molt/molt/verifyservice/verify_runner.go`
- `cockroachdb_molt/molt/verifyservice/raw_table.go`
- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
- `openapi/verify-service.yaml`
- `docs/operator-guide/verify-service.md`

Out of scope:
- adding new config knobs for max jobs, max active jobs, or result retention
- adding `label`
- adding `mode`
- adding `current_table` by parsing free text
- exposing connection information in HTTP requests or responses
- changing source/destination credentials through HTTP

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers default POST `{}` expanding to all configured databases with default schema/table glob matches
- [ ] Red/green TDD covers `databases` as a string, `databases` as an object with `database_match`, `databases` as an array, array entries as strings, array entries as objects, and mixed string/object arrays
- [ ] Red/green TDD covers `default_schema_match` and `default_table_match` inheritance into per-database matches
- [ ] Red/green TDD covers per-database `schema_match` and `table_match` overrides as both strings and arrays
- [ ] Red/green TDD proves request globs are not echoed as results; job responses show concrete matched database names, schema names, and table names where known
- [ ] Red/green TDD proves invalid glob requests return clear `request_validation` errors and do not create jobs
- [ ] Red/green TDD proves object database entries without `database_match` return clear `request_validation` errors and do not create jobs
- [ ] Red/green TDD proves requests matching no configured databases return clear `request_validation` errors and do not create jobs
- [ ] Red/green TDD proves multiple jobs can run in memory concurrently and are stored safely in a thread-safe job map or equivalent
- [ ] Red/green TDD proves `GET /jobs` returns an array of the same job object schema returned by `GET /jobs/{job_id}`
- [ ] Red/green TDD proves `POST /jobs`, `GET /jobs/{job_id}`, and stop responses use the same job object schema
- [ ] Red/green TDD proves job status is derived from per-database statuses and no top-level job error field is required
- [ ] Red/green TDD proves database failures are represented in the matching database entry's `error` field
- [ ] Red/green TDD proves mismatch findings are represented in the matching database entry's `findings` field
- [ ] Red/green TDD covers `rows_checked` per database, including `null` when unknown
- [ ] OpenAPI and operator docs show the curl-style examples and pretty JSON responses from this task
- [ ] `make check` - passes cleanly
- [ ] `make test` - passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` - passes cleanly
- [ ] If this task impacts ultra-long tests or their selection: `make test-long` - passes cleanly (ultra-long-only)
</acceptance_criteria>
