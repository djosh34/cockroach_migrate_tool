## Task: Wrap MOLT verify and fail on log-detected mismatches <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Build the verification wrapper that interprets MOLT verify correctly by parsing its output rather than trusting process exit code alone. The higher order goal is to make repeated parity checks and final cutover verification trustworthy.

In scope:
- execute MOLT verify against the real destination tables
- parse JSON log lines
- fail on mismatches even when process exit code is `0`
- aggregate clear operator-facing results

Out of scope:
- cutover orchestration

This task must use the real target tables, never the helper shadow tables.

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers success and mismatch cases from real MOLT log output
- [ ] The wrapper fails when MOLT logs report row mismatches even if the process exit code is `0`
- [ ] The wrapper checks the real destination tables only
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

