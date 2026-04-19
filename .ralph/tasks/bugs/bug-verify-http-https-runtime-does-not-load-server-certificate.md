## Bug: Verify HTTP HTTPS runtime does not load the configured server certificate <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
The verify HTTP audit found that the HTTPS runtime path builds `server.TLSConfig` without loading `listener.tls.cert_path` and `listener.tls.key_path` into `TLSConfig.Certificates`. `Run(...)` then calls `ListenAndServeTLS("", "")`, which per the standard-library contract requires the certificate to already be present in `TLSConfig` when empty filenames are used.

This was detected during audit pass 5 while reviewing `cockroachdb_molt/molt/verifyservice/runtime.go`, `cockroachdb_molt/molt/verifyservice/config.go`, and the local standard-library contract from `go doc net/http.Server.ListenAndServeTLS`.

This is security-sensitive because operators may believe they have deployed HTTPS successfully when the runtime path is not actually wiring in the configured certificate material. At best the service fails to start; at worst the transport hardening story is misleading and under-tested.

Audit pass: 5

Affected files or boundaries:
- `cockroachdb_molt/molt/verifyservice/runtime.go`
- `cockroachdb_molt/molt/verifyservice/config.go`
- HTTPS bootstrap boundary for `ListenerTLSConfig.ServerTLSConfig()`

First Red test to add:
- add a unit or integration test proving `ListenerTLSConfig.ServerTLSConfig()` or `Run(...)` loads a valid server certificate/key pair and can start HTTPS with `ListenAndServeTLS("", "")`.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [x] I created a Red unit and/or integration test that captures the bug
- [x] I made the test green by fixing
- [x] I manually verified the bug, and created a new Red test if not working still
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
