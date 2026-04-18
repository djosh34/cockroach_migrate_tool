## Task: Compare Cockroach and PostgreSQL schema exports semantically <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Build the schema comparison logic that validates source and destination compatibility semantically rather than by text diff. The higher order goal is to stop bad migrations before CDC starts and to avoid false mismatches caused by dialect formatting differences.

In scope:
- parse or normalize Cockroach schema export
- parse or normalize PostgreSQL schema export
- compare tables, columns, nullability, PKs, FKs, unique constraints, and relevant index structure
- support excluded tables
- produce actionable mismatch output

Out of scope:
- schema generation
- helper bootstrap

This task must reflect the investigation result that raw text diff is too noisy and not acceptable.

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers matching and mismatching schema cases across Cockroach and PostgreSQL exports
- [ ] The comparison is semantic rather than raw text diff
- [ ] Excluded tables are supported cleanly
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

