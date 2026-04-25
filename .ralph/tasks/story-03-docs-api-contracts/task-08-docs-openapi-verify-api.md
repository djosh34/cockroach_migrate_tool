## Task: Create OpenAPI 3.0 specification for verify service HTTP API <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete

**Goal:** Produce an OpenAPI 3.0 YAML specification file that completely describes the verify service HTTP API. This enables API discovery, client generation, and automated documentation. The spec must be accurate to the current implementation in `cockroachdb_molt/molt/verifyservice/service.go` and `cockroachdb_molt/molt/verifyservice/http_test.go`.

**Exact things to include:**
- OpenAPI file at `openapi/verify-service.yaml` (or similar canonical location).
- Server URL placeholder: `http://localhost:8080` (document that actual host/port comes from `listener.bind_addr`).
- All endpoints:
  - `POST /jobs` — start a verify job
  - `GET /jobs/{job_id}` — poll job status and results
  - `POST /jobs/{job_id}/stop` — request job cancellation
  - `POST /tables/raw` — read raw table data (when enabled)
  - `GET /metrics` — Prometheus metrics text
- Path parameters: `job_id` (string).
- Request body schemas:
  - `POST /jobs`: flat filter fields (`include_schema`, `include_table`, `exclude_schema`, `exclude_table` as optional strings).
  - `POST /tables/raw`: `database` (enum: `source` | `destination`), `schema` (string), `table` (string).
  - `POST /jobs/{job_id}/stop`: empty object `{}`.
- Response schemas:
  - `202 Accepted` for `POST /jobs` with `job_id` and `status`.
  - `200 OK` for `GET /jobs/{job_id}` with full job response (running, succeeded, failed, stopped).
  - `200 OK` for `POST /jobs/{job_id}/stop` with `job_id` and `status: stopping`.
  - `200 OK` for `POST /tables/raw` with `database`, `schema`, `table`, `columns`, `rows`.
  - `200 OK` for `GET /metrics` with `text/plain` content.
  - `400 Bad Request` with structured operator error (`category`, `code`, `message`, `details[]`).
  - `404 Not Found` with operator error for unknown job.
  - `409 Conflict` with operator error for already-running job.
  - `403 Forbidden` for raw tables when disabled.
  - `413 Request Entity Too Large` for oversized request bodies.
- Enum values documented:
  - `JobStatus`: `running`, `succeeded`, `failed`, `stopped`
  - Error `category`: `request_validation`, `job_state`, `source_access`, `mismatch`, `verify_execution`
  - Error `code` examples: `unknown_field`, `job_already_running`, `job_not_found`, `connection_failed`, `mismatch_detected`, `verify_failed`
- Example values for each request and response.
- Note that the API is stateful and only retains the most recent completed job.

**Exact things NOT to include:**
- Runner webhook endpoints (`/healthz`, `/ingest/{mapping_id}`).
- Internal Go types (`verifyservice.Job`, `verifyservice.Service`).
- Database connection string schemas or PostgreSQL wire protocol details.
- CockroachDB CDC changefeed details.
- Authentication schemes beyond a note that TLS/mTLS is configured at the listener level.
- Rate limiting or quota information (not implemented).
- WebSocket or streaming endpoints (not implemented).
- Future endpoints or features not yet in the code.
- Implementation notes about mutexes, goroutines, or memory layout.

**End result:**
A user can open `openapi/verify-service.yaml` in Swagger UI, Postman, or any OpenAPI client generator and have a complete, accurate contract for the verify service API.
</description>

<acceptance_criteria>
- [ ] OpenAPI 3.0 YAML file exists and validates against the OpenAPI schema (use a linter)
- [ ] All verify service endpoints are documented with paths, methods, and parameters
- [ ] Request body schemas match the actual JSON decoder expectations (flat filters, raw table request)
- [ ] Response schemas cover all status codes returned by the handlers
- [ ] Enum values for status and error categories are explicitly listed
- [ ] Example values are provided for requests and responses
- [ ] No runner endpoints or internal implementation details are included
- [ ] README references the OpenAPI file location
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
