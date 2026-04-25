## Task: Document verify service job lifecycle and stateful behavior <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete

**Goal:** Add explicit documentation to the README about the verify service's job lifecycle, state retention policy, and polling pattern. Currently the README shows example curl commands but never explains that only one job runs at a time, that completed jobs are evicted except the most recent one, and that all job state is lost when the process restarts. Users discover this through 404 errors.

**Exact things to include:**
- A new subsection under "## Verify Quick Start" titled "Job Lifecycle".
- List of job states with descriptions:
  - `running`: actively verifying
  - `succeeded`: verification completed with no mismatches
  - `failed`: verification completed with mismatches or encountered an error
  - `stopped`: explicitly cancelled via `POST /jobs/{id}/stop`
- Polling pattern guidance: "Poll `GET /jobs/{job_id}` every N seconds until status is no longer `running`".
- Concurrency limit: "Only one job can run at a time. Starting a second job returns HTTP 409 Conflict."
- Retention policy: "Only the most recent completed job is retained. Starting a new job evicts the previous completed job."
- Restart behavior: "Job state is held in memory. If the verify service process restarts, all job history is lost and previous job IDs will return HTTP 404."
- Example of the full lifecycle with curl commands: start, poll running, poll completed, inspect result.
- Guidance on interpreting results: check `result.summary` first, then `result.mismatch_summary`, then `result.findings`.
- Example of a 409 Conflict response body.

**Exact things NOT to include:**
- Implementation details about in-memory storage, mutexes, or goroutines.
- Promises or speculation about future persistence features (e.g., "we may add SQLite storage later").
- Workarounds using external databases or caching layers.
- Kubernetes-specific pod restart semantics (keep it general: "process restart").
- Internal Go struct field names.
- Metrics interpretation (that is a separate topic).
- Authentication details (already covered).

**End result:**
A user reading the "Verify Quick Start" section knows before they start their first job that:
1. They should poll until completion.
2. They cannot start two jobs simultaneously.
3. If they restart the service, old job IDs are gone.
4. Only the latest completed job is queryable.
</description>

<acceptance_criteria>
- [ ] README "Verify Quick Start" contains a "Job Lifecycle" subsection
- [ ] All four job states are documented with plain-language descriptions
- [ ] Polling guidance is explicit and includes a suggested interval
- [ ] Concurrency limit and 409 behavior are documented with example response
- [ ] Retention policy (only most recent completed job kept) is stated clearly
- [ ] Restart amnesia behavior is stated clearly without implementation details
- [ ] Full lifecycle example uses actual curl commands and response bodies
- [ ] No promises of future features or persistence workarounds
- [ ] README operator surface contract test passes
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
