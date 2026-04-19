## Task: Add a dedicated verify-service config with source and destination TLS support and explicit verify mode selection <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a very scoped config contract for the verify service that covers source database connectivity, destination database connectivity, certificate-based auth support, and explicit choice of verify mode. The higher order goal is to make the verify image independently operable without inheriting the runner's config surface or allowing HTTP callers to alter database connection behavior.

In scope:
- separate verify-service config
- source DB and destination DB connection settings
- cert support for both sides
- passwordless/cert-based auth support
- explicit choice between `verify-full` and `verify-ca`
- HTTPS and mTLS listener configuration for the verify service itself as an optional additional authentication layer
- explicit prohibition on changing connection URLs or TLS material through the HTTP API

Out of scope:
- runner config
- source SQL emission

Decisions already made:
- verify gets its own config surface
- both source and destination connections need cert support
- the supported verify modes are `verify-full` and `verify-ca`
- all database connection details must come from config only
- the HTTP API must not accept connection URLs or other connection-detail overrides
- the verify API will often sit behind an authenticated proxy, but it must still support HTTPS mTLS directly as an additional check
- mTLS is recommended and must be explicit in config, but it is not mandatory
- if direct service auth is disabled, the product must state clearly that no extra built-in protection is being provided by the service itself

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers parsing and validating the dedicated verify-service config
- [ ] The config supports source and destination DB settings, TLS material, passwordless certificate-based auth, and optional HTTPS mTLS listener settings
- [ ] The config requires an explicit verify mode of `verify-full` or `verify-ca`, and the HTTP API cannot override connection details from config
- [ ] Configuration and docs make it explicit when direct service authentication is disabled that no extra built-in protection is being provided
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
