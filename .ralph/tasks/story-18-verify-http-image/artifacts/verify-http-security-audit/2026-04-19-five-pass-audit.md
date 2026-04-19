## Verify HTTP Security Audit: Five Passes

### Pass 1

- Scope: request decoding and untrusted input handling for `POST /jobs` and `POST /stop`
- Files and tests reviewed:
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/filter.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
  - `cockroachdb_molt/molt/verifyservice/filter_test.go`
  - targeted tests:
    - `TestPostJobsRejectsConnectionLikeRequestFields`
    - `TestPostJobsRejectsUnknownTopLevelFields`
    - `TestPostJobsRejectsTrailingJSONDocuments`
- Attack hypotheses checked:
  - hostile top-level fields can be silently ignored and create fake override channels
  - multiple JSON documents can smuggle a second payload through the same request body
  - regex input is interpreted as data, not shell or config directives
  - request bodies are unbounded and can force parser-side memory growth
- Findings:
  - Completed narrow hardening: request decoding is now centralized in one `decodeJSONBody(...)` boundary that rejects unknown JSON fields and trailing documents before any handler-specific work starts.
  - No finding on regex-to-shell injection inside this pass; regex strings stay typed data and are compiled with `regexp.CompilePOSIX`.
  - Confirmed issue: request body size is still unbounded. `decodeJSONBody(...)` uses the full request body without `http.MaxBytesReader` or another hard cap, so a hostile client can force large allocations before validation.
- Bug task paths created:
  - `.ralph/tasks/bugs/bug-verify-http-request-body-size-is-unbounded.md`
- Narrow code/test hardening completed during this pass:
  - centralized strict JSON decoding in `verifyservice/service.go`
  - added black-box request-boundary tests in `verifyservice/http_test.go`

### Pass 2

- Scope: process launch, config isolation, and non-shell execution guarantees
- Files and tests reviewed:
  - `cockroachdb_molt/molt/verifyservice/verify_runner.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner_test.go`
  - `cockroachdb_molt/molt/verifyservice/runtime.go`
  - `cockroachdb_molt/molt/verifyservice/config.go`
  - code search for `os/exec`, `exec.Command`, and `sh -c`
- Attack hypotheses checked:
  - request JSON can influence shell commands, argv construction, environment, DB URLs, or TLS file paths
  - verify execution escapes into a shell boundary
  - config-only connection-string derivation has drifted
- Findings:
  - No shell/process-launch finding in the request path. `VerifyRunner.Run(...)` derives both connection strings from config and calls typed Go functions (`dbconn.Connect`, `verify.Verify`) directly.
  - Existing tests already prove the request surface only contributes `utils.FilterConfig`, not URLs or TLS material.
- Bug task paths created:
  - none
- Narrow code/test hardening completed during this pass:
  - none

### Pass 3

- Scope: job lifecycle, cancellation, and resource exhaustion
- Files and tests reviewed:
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/progress.go`
  - `cockroachdb_molt/molt/verifyservice/metrics.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
- Attack hypotheses checked:
  - completed jobs accumulate forever
  - per-job results and mismatch arrays retain unbounded memory
  - `/metrics` collection cost scales with historical jobs and tables
  - write failures can panic the process in a hostile-client scenario
- Findings:
  - Confirmed issue: completed jobs are retained forever in `Service.jobs`, including `statusMessages`, `summaryEvents`, `mismatches`, and `errors`. There is no TTL, cap, or pruning path.
  - Confirmed issue: `/metrics` iterates every remembered job on every scrape, so retention growth becomes scrape amplification and label-cardinality growth.
  - Reviewed `writeJSON(...)` panic behavior. This is undesirable, but in the current `net/http` model it is request-scoped and not the primary high-confidence resource-exhaustion finding compared with the unbounded retention path above.
- Bug task paths created:
  - `.ralph/tasks/bugs/bug-verify-http-retains-completed-jobs-and-metrics-forever.md`
- Narrow code/test hardening completed during this pass:
  - none

### Pass 4

- Scope: result rendering, metrics exposure, and information disclosure
- Files and tests reviewed:
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/progress.go`
  - `cockroachdb_molt/molt/verifyservice/metrics.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
- Attack hypotheses checked:
  - `GET /jobs/{job_id}` reveals operational details to any caller who knows or can guess a job ID
  - `/metrics` reveals database names, schema names, table names, mismatch kinds, and failure/error presence to any caller
  - free-text result strings could be reinterpreted rather than rendered as inert JSON/text
- Findings:
  - No finding on string reinterpretation. Result strings remain inert JSON/text and are not executed.
  - Confirmed issue: the service exposes detailed job results and Prometheus metrics without any authentication or authorization layer. The response surface includes job IDs, timestamps, failure reasons, mismatch details, source and destination database names, schema names, and table names.
  - The current tests intentionally assert that this data is present, so the disclosure is part of the current public behavior, not an accidental omission in one endpoint.
- Bug task paths created:
  - `.ralph/tasks/bugs/bug-verify-http-exposes-job-results-and-metrics-without-auth.md`
- Narrow code/test hardening completed during this pass:
  - none

### Pass 5

- Scope: TLS material handling, listener policy, and runtime bootstrap
- Files and tests reviewed:
  - `cockroachdb_molt/molt/verifyservice/config.go`
  - `cockroachdb_molt/molt/verifyservice/runtime.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - local standard-library reference: `go doc net/http.Server.ListenAndServeTLS`
- Attack hypotheses checked:
  - insecure transport and no-client-auth modes are treated as acceptable defaults for a remotely triggered verify service
  - TLS server bootstrap correctly loads configured certificate material
  - mTLS CA handling fails closed
- Findings:
  - Confirmed issue: insecure listener policy is warning-only. `listener.transport.mode=http` and `listener.tls.client_auth.mode=none` remain valid configurations; the CLI prints a warning but does not enforce a safer default for the remote surface.
  - Confirmed issue: the HTTPS runtime path does not load the configured server certificate/key into `server.TLSConfig`. `Run(...)` calls `ServerTLSConfig()` and then `ListenAndServeTLS("", "")`, while `ServerTLSConfig()` only sets minimum TLS version and optional client CA pool. Per the standard-library contract, empty filenames require `TLSConfig.Certificates` or `GetCertificate` to already be populated.
  - No finding on mTLS CA parse failures; `ServerTLSConfig()` returns an error if the CA file cannot be read or parsed.
- Bug task paths created:
  - `.ralph/tasks/bugs/bug-verify-http-allows-warning-only-insecure-listener-modes.md`
  - `.ralph/tasks/bugs/bug-verify-http-https-runtime-does-not-load-server-certificate.md`
- Narrow code/test hardening completed during this pass:
  - none
