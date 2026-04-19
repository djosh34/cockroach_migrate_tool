## Task: Verify direct Docker build and run works without wrapper scripts <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Create an explicit verification task that proves the user path works directly through `docker build`, `docker run`, or `docker compose up` without wrapper bash scripts. The higher order goal is to ensure the novice-user flow is based on normal container commands rather than hidden helper tooling.

In scope:
- direct container build
- direct container run
- no wrapper shell scripts in the user path

Out of scope:
- advanced development workflows

</description>


<acceptance_criteria>
- [x] Red/green TDD covers direct Docker-based user flow verification
- [x] The task fails if the user path requires wrapper bash scripts
- [x] The documented container commands work as written
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-13-verify-novice-user/02-task-verify-direct-docker-build-and-run-without-wrapper-scripts_plans/2026-04-19-direct-docker-build-run-plan.md</plan>
