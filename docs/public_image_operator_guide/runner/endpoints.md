# Runner: HTTP Endpoints

The runner exposes three HTTP endpoints on the address configured in `webhook.bind_addr`.

## Health check

```
GET /healthz
```

Returns `200 OK` when the runner process is alive and ready.

```bash
curl -k https://runner.example.internal:8443/healthz
```

Use this for container health checks, load-balancer probes, and readiness gates. Use `http://` when `webhook.mode` is `http`.

## Metrics

```
GET /metrics
```

Returns Prometheus-formatted metrics as `text/plain`.

```bash
curl -k https://runner.example.internal:8443/metrics
```

Metrics are prefixed with `cockroach_migration_tool_runner_`.

## Ingest

```
POST /ingest/{mapping_id}
```

This is the endpoint that CockroachDB changefeeds post to. The route is exactly `/ingest/{mapping_id}`, where `{mapping_id}` matches the `id` field in a runner mapping.

CockroachDB changefeeds send batches to this endpoint automatically. You do not call it manually under normal operation.

### Request format

Content-Type: `application/json`

A row batch:

```json
{
  "length": 2,
  "payload": [
    {
      "after": {"id": 1, "email": "first@example.com"},
      "key": {"id": 1},
      "op": "c",
      "source": {"database_name": "demo_a", "schema_name": "public", "table_name": "customers"}
    },
    {
      "key": {"id": 2},
      "op": "d",
      "source": {"database_name": "demo_a", "schema_name": "public", "table_name": "customers"}
    }
  ]
}
```

A resolved watermark:

```json
{"resolved": "1776526353000000000.0000000000"}
```

### Response codes

| Status | Meaning |
| ------ | ------- |
| `200 OK` | Batch accepted |
| `400 Bad Request` | Malformed batch (e.g. length mismatch) |
| `404 Not Found` | Unknown `mapping_id` |
| `500 Internal Server Error` | Processing failure |

### Manual test

You can send a test batch to verify the ingest path is wired end to end:

```bash
curl -k -X POST \
  -H 'content-type: application/json' \
  -d '{"length":1,"payload":[{"after":{"id":1,"name":"test"},"key":{"id":1},"op":"c","source":{"database_name":"demo_a","schema_name":"public","table_name":"customers"}}]}' \
  https://localhost:8443/ingest/app-a
```

A `200` response confirms the runner received the batch for the `app-a` mapping.

## See also

- [Runner getting started](./getting-started.md) — pull, configure, validate, and run
- [Runner configuration](./configuration.md) — full YAML reference
- [CockroachDB source setup](../source-setup/cockroachdb-setup.md) — changefeeds that target `/ingest/{mapping_id}`
- [Troubleshooting](../troubleshooting.md) — common runner errors