## Task: Build the continuous upsert reconcile loop from shadow to real tables <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Build the continuous reconcile loop that copies rows from helper shadow tables into the real constrained tables in dependency order. The higher order goal is to keep the real target tables converging toward the helper shadow truth continuously until cutover.

In scope:
- dependency-ordered upsert passes
- repeatable reconcile execution
- support multiple databases and table mappings
- advance table and stream sync state on success

Out of scope:
- delete reconcile
- MOLT verify

This task must preserve:
- real target tables keep PKs/FKs/indexes enabled
- reconcile runs continuously, not only by manual trigger

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers dependency-ordered upsert reconcile and repeated execution
- [ ] Real tables converge toward helper shadow state through repeated upsert passes
- [ ] Successful upsert reconcile updates sync state in PostgreSQL helper tables
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

