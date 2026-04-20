## Task: Verify CLI command complexity stays low and `--help` works everywhere a user would expect it <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Add a user-test task that verifies the command surface stays simple enough for a novice operator following the README. The higher order goal is to prevent the images from growing deep command trees and hard-to-discover flags that make the README path brittle and hard to follow.

In scope:
- describe and verify the complexity of the user-facing commands used in the README
- prefer a command shape close to `cli-name [one action level] [--args]` wherever practical
- reject unnecessary nested subcommand depth where a flatter command shape would work
- verify `--help` works when appended to any supported command or subcommand the user may need
- verify help output is sufficient for operator use and aligns with the README examples

Out of scope:
- enforcing a rigid one-size-fits-all CLI rule when a slightly deeper command shape is truly justified

Decisions already made:
- minimal subcommand complexity is a design goal
- `cli-name [one action level] [--args]` is the target shape whenever practical
- this is not an absolute rule, but the burden is on deeper command trees to justify themselves
- `--help` must always work when appended to any command the user may invoke
- any issue found during this verification must immediately create a bug via the `add-bug` skill
- when a bug is found, the verification flow must ask for a task switch so the system can switch to the bug task
- this task must not be marked passed unless the verification finishes with zero new bug tasks created

</description>


<acceptance_criteria>
- [x] Red/green TDD covers command-shape simplicity and `--help` behavior across the supported user-facing commands
- [x] The task fails if the supported CLI surface grows unnecessary nested action levels for novice-user flows
- [x] The task proves `--help` works on every supported command or subcommand used by the README path
- [x] The task proves the help output is consistent with the README examples and sufficient for a novice operator
- [x] Every issue found during verification immediately results in a new bug task created via `add-bug`, and the workflow asks for a task switch to that bug
- [x] `<passes>true</passes>` is allowed only if the verification completes perfectly with no new bug task required
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-24-readme-only-novice-e2e/03-task-verify-cli-command-complexity-stays-low-and-help-works-everywhere_plans/2026-04-20-cli-help-simplicity-plan.md</plan>
