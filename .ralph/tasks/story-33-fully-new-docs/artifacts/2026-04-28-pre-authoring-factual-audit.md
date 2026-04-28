# Pre-Authoring Factual Audit

Timestamp: `2026-04-28T23:55:43+02:00`

## Exact opencode-go models available

Command:

```bash
opencode models | grep '^opencode-go/'
```

Relevant results:

- `opencode-go/glm-5.1`
- `opencode-go/deepseek-v4-flash`
- `opencode-go/kimi-k2.6`

## Published image contract

Resolved contract:

- Primary published registry: GHCR
- Final GHCR tags:
  - `ghcr.io/<owner>/runner-image:<git-sha>`
  - `ghcr.io/<owner>/verify-image:<git-sha>`
- Quay is a post-publish mirror of those GHCR images, not the primary source of truth.

Evidence:

- `.github/workflows/publish-images.yml`
  - `publish-multiarch` logs into `ghcr.io`
  - runs `./scripts/ci/publish-ghcr-multiarch-from-archives.sh`
  - only after that runs `./scripts/ci/publish-quay-from-ghcr.sh`
- `scripts/ci/publish-ghcr-multiarch-from-archives.sh`
  - sets `registry_prefix="ghcr.io/${ghcr_owner}"`
  - publishes final refs as `${registry_prefix}/${image_name}:${git_sha}`
  - iterates `image_name` over `runner-image` and `verify-image`
- `scripts/ci/publish-quay-from-ghcr.sh`
  - requires the GHCR publish summary as input
  - copies existing GHCR refs to Quay with `skopeo copy --all`
- `flake.nix`
  - `verify-image = pkgs.dockerTools.buildImage { name = "verify-image"; ... }`
  - `runner-image = pkgs.dockerTools.buildImage { name = "runner-image"; ... }`

Important contradiction to avoid reproducing:

- `README.md` still says:
  - `ghcr.io/${GITHUB_OWNER}/cockroach-migrate-runner:${IMAGE_TAG}`
  - `ghcr.io/${GITHUB_OWNER}/cockroach-migrate-verify:${IMAGE_TAG}`
- That naming is stale against the actual publish scripts and flake image names.

## Runner public contract

Authoritative files:

- `crates/runner/src/lib.rs`
- `crates/runner/src/webhook_runtime/mod.rs`
- `crates/ingest-contract/src/lib.rs`

Verified CLI surface:

- `validate-config --config <PATH> [--deep]`
- `run --config <PATH>`
- global optional flag: `--log-format text|json`

Verified semantics:

- plain `validate-config` is offline config validation
- `validate-config --deep` also verifies destination connectivity and schema
- `run` starts both the webhook runtime and reconcile runtime

Verified HTTP surface:

- `GET /healthz`
- `GET /metrics`
- `POST /ingest/{mapping_id}`

Verified webhook notes:

- the route path is `/ingest/{mapping_id}`
- the listener supports HTTP or HTTPS
- HTTPS may require client certificates when `webhook.tls.client_ca_path` is set

## Verify-service public contract

Authoritative files:

- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
- `cockroachdb_molt/molt/verifyservice/service.go`
- `openapi/verify-service.yaml`

Verified CLI surface:

- entry command: `verify-service`
- subcommands:
  - `validate-config --config <path>`
  - `run --config <path>`
- optional flag on each subcommand:
  - `--log-format text|json`

Verified HTTP/API surface:

- `POST /jobs`
- `GET /jobs/{job_id}`
- `POST /jobs/{job_id}/stop`
- `GET /metrics`

Verified API semantics:

- default local URL in OpenAPI: `http://localhost:8080`
- actual host and port come from `listener.bind_addr`
- only one active job may run at a time
- only the most recent completed job is retained
- retained job state is lost on process restart
- stop is a two-stage lifecycle:
  - stop request returns `stopping`
  - terminal lifecycle becomes `stopped`

## Setup SQL contract

Authoritative files:

- `scripts/generate-cockroach-setup-sql.sh`
- `scripts/generate-postgres-grants-sql.sh`
- `scripts/README.md`
- `docs/setup_sql/cockroachdb-source-setup.md`
- `docs/setup_sql/postgresql-destination-grants.md`

Verified Cockroach setup facts:

- one source database can contain multiple mappings
- the operator must:
  - enable `kv.rangefeed.enabled`
  - capture `SELECT cluster_logical_timestamp() AS changefeed_cursor;`
  - create one webhook `CREATE CHANGEFEED` per mapping
- each generated changefeed targets:
  - `webhook-<base_url>/ingest/<mapping_id>?ca_cert=<percent-encoded-base64-cert>`
- required changefeed options:
  - `cursor = '__CHANGEFEED_CURSOR__'`
  - `initial_scan = 'yes'`
  - `envelope = 'enriched'`
  - `resolved = '<interval>'`

Verified PostgreSQL grants facts:

- the operator must grant:
  - `GRANT CONNECT, CREATE ON DATABASE <database> TO <runtime_role>;`
  - `GRANT USAGE ON SCHEMA <schema> TO <runtime_role>;`
  - `GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE <schema>.<table> TO <runtime_role>;`
- the runtime creates `_cockroach_migration_tool` and its helper tables itself after grants exist

## TLS/config guidance

Authoritative files:

- `docs/tls-configuration.md`
- `README.md`
- runtime code and tests for runner and verify-service

Verified TLS facts:

- runner webhook listener:
  - `mode: http` or `mode: https`
  - server TLS files:
    - `webhook.tls.cert_path`
    - `webhook.tls.key_path`
  - optional client-auth CA:
    - `webhook.tls.client_ca_path`
- runner destination and verify DB connections may use URL `sslmode` and nested TLS file-path fields
- verify listener host/port come from `listener.bind_addr`
- verify listener HTTPS uses cert/key under `listener.tls.*`
- verify listener mTLS uses `listener.tls.client_ca_path`

## Validation lanes required for task completion

- `make check`
- `make lint`
- `make test`

Must not run for this task:

- `make test-long`

