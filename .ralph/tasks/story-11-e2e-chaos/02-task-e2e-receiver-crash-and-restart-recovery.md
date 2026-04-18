## Task: End-to-end test receiver crash and restart recovery <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a real end-to-end test where the destination runner crashes after helper-state persistence or during reconcile and then recovers correctly. The higher order goal is to prove the chosen helper-shadow architecture is restartable under realistic failure.

In scope:
- runner crash after helper persistence
- runner crash during reconcile
- restart behavior
- eventual correctness of real tables

Out of scope:
- source-side manual rescue commands

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers crash-and-restart recovery end to end
- [ ] No extra source-side commands are used to rescue the migration after CDC setup
- [ ] Real target tables converge correctly after restart
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

