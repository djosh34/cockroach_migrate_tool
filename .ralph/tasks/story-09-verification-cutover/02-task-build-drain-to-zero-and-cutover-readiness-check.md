## Task: Build drain-to-zero and cutover readiness checks <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Build the runtime checks that determine whether the system is ready for cutover after writes are blocked at the API layer. The higher order goal is to turn the selected handover model into something the operator can trust and observe.

In scope:
- detect whether received watermarks have been reconciled
- detect whether helper-state and real-table sync has drained to zero
- expose cutover readiness state
- integrate MOLT verification result

Out of scope:
- actual application traffic switching

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers readiness false/true conditions based on watermarks and verification state
- [ ] The system can report whether the migration has drained to zero and is ready for final cutover
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

