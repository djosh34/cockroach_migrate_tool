## Task: Produce an exhaustive novice-user manual experience report <status>completed</status> <passes>true</passes>

<description>
**Goal:** Manually try the whole system as a novice user and produce a very exhaustive, deeply investigative Markdown report of the actual experience. The higher order goal is to measure operator friction honestly from the user's perspective rather than from the implementer's assumptions.

This task must explicitly simulate a novice user mindset:
- do not rely on prior author knowledge
- do not silently fill in missing documentation from reading code unless the report clearly marks that the README path failed and extra investigation was required
- do not treat confusion or friction as acceptable

The report must focus on:
- exact step-by-step operator flow
- where the user hesitated
- whether the README alone was enough
- whether anything had to be looked up elsewhere
- whether commands were too long, awkward, or fragile
- whether the config was obvious or confusing
- whether build/run/bootstrap/cutover concepts felt simple or intimidating
- what the total perceived friction was
- what should be simplified

The task must produce report artifacts inside this story directory itself, under:
- `.ralph/tasks/story-14-reports/artifacts/report-novice-user/`

Required artifact files at minimum:
- `summary.md`
- `step-by-step-experience.md`
- `friction-log.md`
- `recommendations.md`

The report must be extremely detailed and comprehensive. Every meaningful stumble, ambiguity, and cognitive burden must be written down, even if it seems small.

In scope:
- manual novice-user trial of the full documented path
- README-first evaluation
- exact commands used
- exact points of confusion
- explicit verdict on whether the quick start is actually novice-usable

Out of scope:
- changing implementation code directly as part of this task unless strictly necessary to complete the report workflow

This task must stand on its own and evaluate the current system as implemented at the time it is run.

</description>


<acceptance_criteria>
- [x] The report is based on an actual manual novice-user trial, not a hypothetical writeup
- [x] The required artifact files are produced in `.ralph/tasks/story-14-reports/artifacts/report-novice-user/`
- [x] The report explicitly states whether the README alone was sufficient, and if not, exactly where and why it failed
- [x] The report records every command used and every point where the user needed to pause, infer, investigate, or look elsewhere
- [x] The report includes concrete simplification recommendations prioritized by actual user friction
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
