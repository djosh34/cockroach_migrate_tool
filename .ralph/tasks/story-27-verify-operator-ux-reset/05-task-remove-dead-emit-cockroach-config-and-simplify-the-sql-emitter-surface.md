## Task: Remove dead emit-cockroach config and simplify the SQL-emitter surface <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Remove config fields from `setup-sql emit-cockroach-sql` that are required today but not actually used to render the SQL payload, starting with the dead `cockroach.url` field. The higher order goal is to stop the SQL emitter from pretending it needs live source-connection details when generation is really an offline rendering step.

Current product gap from the 2026-04-20 user review:
- `emit-cockroach-sql` requires non-used config
- `cockroach.url` makes it look like SQL generation depends on real Cockroach access even though the render path does not connect anywhere
- dead config creates false operator requirements and increases confusion

In scope:
- remove `cockroach.url` from the `emit-cockroach-sql` config contract if it is still unused at render time
- remove any other emit-only config that is parser-required but not actually consumed by rendering
- update the SQL renderer output so it no longer prints dead config into comments
- update README examples, fixtures, contract tests, and novice-user docs to match the simplified emitter config
- make the remaining emitter config reflect only data that is actually needed to render the SQL artifact, such as webhook details and selected source tables

Out of scope:
- changing the actual `CREATE CHANGEFEED` SQL semantics beyond what is required to keep the emitter honest
- redesigning the runner or verify runtime

Decisions already made:
- generating SQL must stay an offline render step and must not imply live source access when none is required
- this is a greenfield project with no backwards-compatibility promise, so dead config should be deleted rather than deprecated
- the simplified config should make it obvious which fields are used to build the output and which are not

Relevant files and boundaries:
- `crates/setup-sql/src/config/cockroach.rs`
- `crates/setup-sql/src/config/cockroach_parser.rs`
- `crates/setup-sql/src/render/cockroach.rs`
- `crates/setup-sql/src/lib.rs`
- `crates/setup-sql/tests/bootstrap_contract.rs`
- `crates/setup-sql/tests/fixtures/valid-cockroach-setup-config.yml`
- `crates/setup-sql/tests/fixtures/readme-cockroach-setup-config.yml`
- `README.md`
- `crates/runner/tests/readme_operator_surface_contract.rs`

</description>


<acceptance_criteria>
- [ ] Red/green TDD proves `emit-cockroach-sql` renders successfully without dead connection-only config such as `cockroach.url`
- [ ] The parser rejects only fields that are actually needed for rendering; non-used required config is removed
- [ ] The rendered SQL artifact no longer includes dead config comments that imply fake runtime requirements
- [ ] README examples and fixture configs use the simplified emit config and stay copy-pasteable
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
