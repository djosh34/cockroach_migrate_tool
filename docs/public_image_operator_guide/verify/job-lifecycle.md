# Verify-Service: Job Lifecycle

The verify-service exposes an HTTP API to start, poll, and stop verification jobs. This page describes every endpoint and the job state machine.

## Key constraints

> **Only one job** can run at a time. Starting a second returns `409 Conflict`.
>
> **Only the most recent completed job** is retained. Starting a new job evicts previous results.
>
> **Job state is in-memory.** If the process restarts, all job IDs return `404 Not Found`.

## Endpoints

### Start a verify job

```
POST /jobs
Content-Type: application/json
```

Request body (optional filters):

```json
{
  "include_schema": "^public$",
  "include_table": "^(accounts|orders)$"
}
```

All fields are optional POSIX regular expressions:

| Field | Description |
| ----- | ----------- |
| `include_schema` | Include schemas matching this regex |
| `include_table` | Include tables matching this regex |
| `exclude_schema` | Exclude schemas matching this regex |
| `exclude_table` | Exclude tables matching this regex |

To verify everything, send an empty object:

```json
{}
```

**Success —** `202 Accepted`:

```json
{"job_id": "job-000001", "status": "running"}
```

**Already running —** `409 Conflict`:

```json
{"error": {"category": "job_state", "code": "job_already_running", "message": "a verify job is already running"}}
```

### Poll job status

```
GET /jobs/{job_id}
```

```bash
curl --silent --show-error --insecure "${VERIFY_API}/jobs/${JOB_ID}"
```

**While running —** `200 OK`:

```json
{"job_id": "job-000001", "status": "running"}
```

**On success —** `200 OK`:

```json
{
  "job_id": "job-000001",
  "status": "succeeded",
  "result": {
    "summary": {
      "tables_verified": 1,
      "tables_with_data": 1,
      "has_mismatches": false
    },
    "table_summaries": [
      {
        "schema": "public",
        "table": "accounts",
        "num_verified": 7,
        "num_success": 7,
        "num_missing": 0,
        "num_mismatch": 0,
        "num_column_mismatch": 0,
        "num_extraneous": 0,
        "num_live_retry": 0
      }
    ],
    "findings": [],
    "mismatch_summary": {
      "has_mismatches": false,
      "affected_tables": [],
      "counts_by_kind": {}
    }
  }
}
```

**On failure — mismatches detected** — `200 OK`:

```json
{
  "job_id": "job-000001",
  "status": "failed",
  "failure": {
    "category": "mismatch",
    "code": "mismatch_detected",
    "message": "verify detected mismatches in 1 table",
    "details": [{"reason": "mismatch detected for public.accounts"}]
  },
  "result": {
    "summary": {
      "tables_verified": 1,
      "tables_with_data": 1,
      "has_mismatches": true
    },
    "table_summaries": [
      {
        "schema": "public",
        "table": "accounts",
        "num_verified": 7,
        "num_success": 6,
        "num_missing": 0,
        "num_mismatch": 0,
        "num_column_mismatch": 1,
        "num_extraneous": 0,
        "num_live_retry": 0
      }
    ],
    "findings": [
      {
        "kind": "mismatching_column",
        "schema": "public",
        "table": "accounts",
        "primary_key": {"id": "101"},
        "mismatching_columns": ["balance"],
        "source_values": {"balance": "17"},
        "destination_values": {"balance": "23"},
        "info": ["balance mismatch"]
      }
    ],
    "mismatch_summary": {
      "has_mismatches": true,
      "affected_tables": [{"schema": "public", "table": "accounts"}],
      "counts_by_kind": {"mismatching_column": 1}
    }
  }
}
```

**On failure — source access error** — `200 OK`:

```json
{
  "job_id": "job-000001",
  "status": "failed",
  "failure": {
    "category": "source_access",
    "code": "connection_failed",
    "message": "source connection failed: dial tcp 127.0.0.1:5432: connect: connection refused",
    "details": [{"reason": "dial tcp 127.0.0.1:5432: connect: connection refused"}]
  }
}
```

**Job not found —** `404 Not Found`:

```json
{"error": {"category": "job_state", "code": "job_not_found", "message": "job not found"}}
```

### Stop a running job

```
POST /jobs/{job_id}/stop
Content-Type: application/json
```

The request body must be an empty JSON object: `{}`

```bash
curl --silent --show-error --insecure \
  -H 'content-type: application/json' \
  -d '{}' \
  -X POST "${VERIFY_API}/jobs/${JOB_ID}/stop"
```

Immediate response — `200 OK`:

```json
{"job_id": "job-000001", "status": "stopping"}
```

> The job transitions from `stopping` to `stopped` asynchronously. Poll `GET /jobs/{job_id}` until `status` is `stopped`.

### Metrics

```
GET /metrics
```

Returns Prometheus-formatted metrics as `text/plain`.

```bash
curl --silent --show-error --insecure "${VERIFY_API}/metrics"
```

Metrics are prefixed with `cockroach_migration_tool_verify_`.

## Job states

| Status | Meaning | Terminal |
| ------ | ------- | -------- |
| `running` | Job is actively verifying | no |
| `stopping` | Stop requested, winding down | no |
| `succeeded` | Verification completed, no mismatches | yes |
| `failed` | Verification completed with mismatches or an error | yes |
| `stopped` | Job was cancelled by operator | yes |

## Interpreting results

1. Check `result.summary.has_mismatches` first.
2. If `true`, inspect `result.mismatch_summary.affected_tables` for the list of affected tables.
3. For each affected table, check `result.findings` for per-row detail including `mismatching_columns`, `source_values`, and `destination_values`.

## Error categories

| Category | When it occurs |
| -------- | -------------- |
| `request_validation` | Invalid filter, unknown field, or body too large |
| `job_state` | Job already running, job not found |
| `source_access` | Cannot connect to source database |
| `mismatch` | Mismatches were detected during verification |
| `verify_execution` | Internal verify execution failure |

## Obsolete job IDs after restart

Because job state lives in memory, restarting the verify-service process means all previous job IDs return `404 Not Found`. There is no persistent job history.

## See also

- [Verify getting started](./getting-started.md) — pull, configure, validate, and run
- [Verify configuration](./configuration.md) — full YAML reference
- [Troubleshooting](../troubleshooting.md) — common verify-service errors