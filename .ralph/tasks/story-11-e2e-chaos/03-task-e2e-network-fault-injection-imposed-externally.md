## Task: End-to-end test externally imposed network fault injection <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a real end-to-end test where network faults are imposed from outside the runner, not via hidden test hooks inside the binary. The higher order goal is to prove the system handles transport instability honestly.

In scope:
- external network fault injection
- retry/resume behavior
- continuous reconcile catch-up after recovery

Out of scope:
- binary-level test-only network stubs

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers externally imposed network instability end to end
- [ ] No hidden in-binary test shortcut is used for network chaos
- [ ] Real target tables converge after the network recovers
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

