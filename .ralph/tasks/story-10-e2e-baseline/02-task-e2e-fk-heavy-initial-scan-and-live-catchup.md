## Task: End-to-end test FK-heavy initial scan and live catch-up <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a real end-to-end test for FK-heavy schemas that proves the selected helper-shadow plus reconcile design handles initial scan and live catch-up while the real tables keep constraints enabled. The higher order goal is to validate the main reason this design was chosen over direct apply.

In scope:
- parent/child/grandchild schema
- initial scan
- live updates during shadowing
- repeated reconcile
- MOLT verify on real tables

Out of scope:
- external chaos injection

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers a real FK-heavy migration from initial scan through live catch-up
- [ ] Real target tables keep PK/FK constraints enabled throughout the test
- [ ] MOLT verify confirms the real target tables match source state
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

