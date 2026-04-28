## Bug: Fix nix flake check without Docker test assumptions <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
`nix flake check` is currently not fully green. The known failures include both verify-binary tests and some long-test related failures.

The end goal is to fully fix `nix flake check`.

Do not run `nix flake check` while creating or triaging this task; this task exists so the failure can be fixed later in a focused pass.

The solution must be Nix-native. Do not use Docker, Docker images, Docker Compose, container runtimes, or any Docker-backed test path to fix this. If any code, test, task, workflow, documentation, or naming assumes or pretends there are still Docker tests, refactor it so the project is explicit that there is no Docker or image-based test surface.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [x] I identified every failing `nix flake check` component, including both verify-binary tests and long-test related failures
- [x] I created a Red unit and/or integration test that captures the first concrete bug before fixing it
- [x] I made that test green by fixing the underlying implementation
- [x] I repeated Red-Green TDD one failing behavior at a time until the known verify-binary and long-test failures are fixed
- [x] I removed or refactored all code, tests, workflows, documentation, and names that assume Docker tests, Docker images, or image-based test execution still exist
- [x] I did not use Docker, Docker Compose, Docker images, or any container runtime as the fix or verification path
- [x] All test execution and service setup required by this fix uses Nix-native mechanisms only
- [x] I manually verified the bug, and created a new Red test if not working still
- [x] `nix flake check` — passes cleanly
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/bugs/bug-fix-nix-flake-check-without-docker_plans/2026-04-28-nix-flake-check-without-docker-plan.md</plan>
