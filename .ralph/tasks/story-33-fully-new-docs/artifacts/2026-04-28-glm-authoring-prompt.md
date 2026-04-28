You are `opencode-go/glm-5.1`, and you are the sole documentation author for this task.

Write a brand-new operator documentation set under exactly this directory:

- `docs/public_image_operator_guide/`

Constraints:

- You have full artistic freedom over structure, wording, headings, examples, diagrams, section order, and narrative flow.
- You must not rely on any other repository doc being read alongside your output.
- The docs must stand on their own for an outside operator who wants to use the project directly from the published Docker images.
- Do not write outside `docs/public_image_operator_guide/`.
- Output a file manifest with complete file contents for every file you want to create.
- Use this exact output format for each file:

```text
=== FILE: docs/public_image_operator_guide/<relative-path>.md ===
<full file contents>
=== END FILE ===
```

- Do not wrap the whole response in JSON.
- Do not omit any file contents.

Non-negotiable factual constraints:

- The primary published image source is GHCR.
- Use the real final image names:
  - `ghcr.io/<owner>/runner-image:<git-sha>`
  - `ghcr.io/<owner>/verify-image:<git-sha>`
- Quay is a mirror of GHCR, not the primary source of truth.
- Do not use stale image names like `cockroach-migrate-runner` or `cockroach-migrate-verify`.
- The docs must cover the complete operator path from published image to verification.

Verified product facts you may rely on:

1. Runner images and CLI

- The runner image is published as `runner-image`.
- Runner CLI:
  - `validate-config --config <PATH> [--deep]`
  - `run --config <PATH>`
  - optional global `--log-format text|json`
- `validate-config` is offline by default.
- `validate-config --deep` also verifies destination connectivity and schema.
- Runner HTTP endpoints:
  - `GET /healthz`
  - `GET /metrics`
  - `POST /ingest/{mapping_id}`
- The route contract is exactly `/ingest/{mapping_id}`.

2. Verify-service image, CLI, and API

- The verify image is published as `verify-image`.
- The process entrypoint command is `verify-service`.
- Verify-service CLI subcommands:
  - `validate-config --config <path>`
  - `run --config <path>`
- Each subcommand accepts `--log-format text|json`.
- Verify-service HTTP API:
  - `POST /jobs`
  - `GET /jobs/{job_id}`
  - `POST /jobs/{job_id}/stop`
  - `GET /metrics`
- OpenAPI default local URL is `http://localhost:8080`, but the real host/port come from `listener.bind_addr`.
- Only one verify job can run at a time.
- Only the most recent completed job is retained.
- Job retention is in-memory and is lost on process restart.
- A stop request returns `stopping` before terminal `stopped`.

3. Source CockroachDB setup

- Operators must prepare source-side SQL before running the runtime.
- Required sequence:
  - enable `SET CLUSTER SETTING kv.rangefeed.enabled = true;`
  - for each source database, run `SELECT cluster_logical_timestamp() AS changefeed_cursor;`
  - create one webhook `CREATE CHANGEFEED` per mapping
- The generated sink form is:
  - `webhook-<base_url>/ingest/<mapping_id>?ca_cert=<percent-encoded-base64-cert>`
- Required changefeed options:
  - `cursor = '<captured decimal cursor>'`
  - `initial_scan = 'yes'`
  - `envelope = 'enriched'`
  - `resolved = '<interval>'`

4. Destination PostgreSQL grants

- Operators must grant:
  - `GRANT CONNECT, CREATE ON DATABASE <database> TO <runtime_role>;`
  - `GRANT USAGE ON SCHEMA <schema> TO <runtime_role>;`
  - `GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE <schema>.<table> TO <runtime_role>;`
- The runtime creates `_cockroach_migration_tool` and its helper tables itself after grants are in place.

5. TLS and configuration facts

- Runner webhook listener:
  - `mode: http` or `mode: https`
  - `webhook.tls.cert_path`
  - `webhook.tls.key_path`
  - optional `webhook.tls.client_ca_path` for client certificates
- Runner destination DBs and verify DBs may use URL `sslmode` plus nested TLS file-path fields.
- Verify listener bind address is configured through `listener.bind_addr`.
- Verify listener HTTPS uses `listener.tls.cert_path` and `listener.tls.key_path`.
- Verify listener mTLS uses `listener.tls.client_ca_path`.

6. Scope expectations

- Your docs should help an operator:
  - identify the right published image references
  - prepare source CockroachDB setup SQL
  - prepare destination PostgreSQL grants
  - write runner config
  - validate and run runner from the published image
  - write verify-service config
  - validate and run verify-service from the published image
  - start, poll, and stop verify jobs
  - understand health/metrics endpoints
  - troubleshoot the most likely setup failures

Author the docs now.
