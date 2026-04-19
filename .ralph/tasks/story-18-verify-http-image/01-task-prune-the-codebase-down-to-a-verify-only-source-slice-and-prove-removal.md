## Task: Prune the codebase down to the verify-only source slice and prove all unrelated code was removed <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Split out the verify image by identifying the minimum package set needed to build verify, then aggressively removing unrelated source outside that slice. The higher order goal is to stop shipping a mixed codebase when the verify image only needs a narrow subset of functionality.

In scope:
- identify the Go packages and internal code needed to build verify
- remove all other code from the verify-image build path
- verify removal so dead or legacy code is not silently retained
- update tests to prove the verify source slice is intentionally minimal

Out of scope:
- adding the HTTP service around verify
- runner image behavior

Decisions already made:
- the verify image must build from MOLT verify source only
- all other source outside verify should be removed from that path aggressively
- this project is greenfield, so no backwards compatibility or legacy preservation applies

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers identification of the minimal verify build slice and proof of unrelated-code removal
- [ ] The verify image build path contains only packages required for verify behavior
- [ ] Automated checks fail if removed unrelated code creeps back into the verify image path
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
