## Bug: Nix test-long misses docker runtime input <status>not_started</status> <passes>false</passes> <priority>high</priority>

<description>
`nix build --no-link .#packages.aarch64-linux.runner-long-test` fails because the long-lane Nix test derivation does not provide a `docker` executable, while the ignored long-lane tests attempt to run Docker.

Detected on 2026-04-28 while comparing Nix artifact lanes and timings. The failure happens immediately across all 18 ignored long-lane tests. Representative log line:

`docker run cockroach cert create-ca should start: No such file or directory (os error 2)`

The failing derivation was `/nix/store/9r6sww5hy406h4iyqb863f8kypqb6d7w-runner-long-test-test-0.1.0.drv`.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [ ] I created a Red unit and/or integration test that captures the bug
- [ ] I made the test green by fixing
- [ ] I manually verified the bug, and created a new Red test if not working still
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
