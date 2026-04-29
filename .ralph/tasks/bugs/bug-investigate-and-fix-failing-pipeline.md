## Bug: Investigate and Fix Failing Pipeline <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
The Product Owner reported on 2026-04-29 that the pipeline is failing. The reported failure came after Ralph Bot previously recorded local `make check`, `make test`, and `make lint` as passing, so the failure is likely in the hosted or pipeline-specific path rather than the default local gates.

Investigate the failing pipeline from the authoritative pipeline logs, identify the exact failing job and command, and fix the underlying cause. Do not guess from local-only output.
</description>

<mandatory_red_green_tdd>
This is a pipeline/workflow bug. Do not use Rust Red-Green TDD unless the investigation proves the failing behavior is in Rust application code and can be captured by a focused unit or integration test.

For workflow, Docker, Nix, or CI configuration failures, reproduce the failing command locally when possible and verify the fix against the same class of pipeline evidence, such as authenticated workflow logs or the exact Nix-backed `make` target used by CI.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [x] I inspected the authoritative failing pipeline logs and recorded the failing workflow/job/command in the task notes or implementation summary
- [x] I identified whether the failure is workflow, Docker/image, Nix, dependency/cache, infrastructure, or Rust application behavior
- [x] If Rust application behavior is responsible: I created a Red unit and/or integration test that captures the bug
- [x] If workflow, Docker, Nix, dependency/cache, or infrastructure behavior is responsible: I reproduced or directly verified the relevant failing command/log path without adding irrelevant Rust tests
- [x] I fixed the root cause of the pipeline failure
- [x] I manually verified the failing pipeline path or equivalent command no longer fails
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly
- [x] `make lint` — passes cleanly
- [x] If this bug impacts ultra-long tests, image publishing, or CI selection: `make test-long` and/or the affected image/pipeline command passes cleanly
</acceptance_criteria>
