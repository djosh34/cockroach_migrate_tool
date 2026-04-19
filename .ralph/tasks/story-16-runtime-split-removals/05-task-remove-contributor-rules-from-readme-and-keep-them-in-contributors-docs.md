## Task: Remove contributor-only coding rules from README and keep the operator path free of internal project structure assumptions <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Remove contributor-only material from the README so novice users are not forced to read project coding rules, repo structure notes, or contributor workflow guidance just to operate the images. The higher order goal is to separate operator UX from contributor guidance cleanly.

In scope:
- delete contributor-only rules from README
- move or keep contributor rules in the proper contributors documentation file
- add checks that the novice-user path does not depend on internal development guidance

Out of scope:
- broad contributor docs redesign beyond moving the required material out of README

Decisions already made:
- novice users must never need to read project coding rules or structure notes
- that material belongs in contributor documentation, not in README
- README must stay operator-focused

</description>


<acceptance_criteria>
- [x] Red/green TDD covers removal of contributor-only guidance from README
- [x] README does not require novice users to understand repository internals or coding rules
- [x] Contributor-only rules are preserved in the appropriate contributors documentation instead
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — not required; this task did not change the long-lane selection boundary
</acceptance_criteria>

<plan>.ralph/tasks/story-16-runtime-split-removals/05-task-remove-contributor-rules-from-readme-and-keep-them-in-contributors-docs_plans/2026-04-19-contributor-doc-boundary-plan.md</plan>
