## Task: Verify the README alone enables a full public-image migration with zero repo access <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a stricter novice-user end-to-end verification story that proves a user can complete a full independent data migration by reading only the README and having nothing installed except Docker. The higher order goal is to make the README and published images the actual product interface rather than a thin layer on top of hidden repository knowledge.

In scope:
- the novice user may read only the README
- the novice user must have zero installed dependencies except Docker
- the novice user must be able to pull the published images and use all of them
- the novice user must do a full independent data migration using only the config snippets and Docker Compose text that appear inline in the README
- zero repository download, zero `git pull`, zero repo searching, and zero local `docker build`; any of those are direct failure conditions
- the novice user must first read the Cockroach SQL output and apply only that exact SQL
- the novice user must then read the PostgreSQL SQL output and apply only that exact SQL with no manual alterations
- the novice user must know the exact required config parameters only from the README and must never need to consult anything else
- the documented setup for this verification path uses secure mTLS-authenticated CockroachDB and PostgreSQL connections
- the novice user must then create and run the runner from publicly available pulled images only
- the runner must directly work from the README-guided setup, and wrong config must fail clearly with operator-usable error messages
- auth failures must be reported clearly, and connection failures must not be misleadingly reported when the real issue is authentication
- the novice user must then deploy and use verification through the HTTP API based only on the inline README guide

Out of scope:
- contributor onboarding
- repository-internal exploration as a fallback
- any workaround that depends on unpublished images or local builds

Decisions already made:
- this is an additional story beyond the earlier registry-only novice-user verification
- the README is the only allowed document for this task
- only publicly available images may be used
- searching for the latest image tag is allowed, but nothing else in the repo may be consulted
- the migration path must explicitly go Cockroach SQL first, then PostgreSQL SQL, then runner, then verify API
- secure setup for this path uses mTLS on both CockroachDB and PostgreSQL
- a wrong config must fail clearly enough for a novice operator to understand what to fix
- any issue found during this verification must immediately create a bug via the `add-bug` skill
- when a bug is found, the verification flow must ask for a task switch so the system can switch to the bug task
- this task must not be marked passed unless the verification finishes with zero new bug tasks created

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers a README-only novice-user path for a full independent migration using only public images
- [ ] The task fails if the user must download the repo, run `git`, search repository files, build images locally, or consult any document besides the README
- [ ] The task proves the user can apply the exact emitted Cockroach SQL first and the exact emitted PostgreSQL SQL second, without manual SQL rewriting
- [ ] The task proves the user can derive the required secure mTLS config values from the README alone and run the runner successfully from pulled public images
- [ ] The task proves wrong config produces clear operator-facing failures, including clean distinction between authentication and connectivity problems
- [ ] The task proves the user can deploy and use the verify HTTP API from the README alone
- [ ] Every issue found during verification immediately results in a new bug task created via `add-bug`, and the workflow asks for a task switch to that bug
- [ ] `<passes>true</passes>` is allowed only if the verification completes perfectly with no new bug task required
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
