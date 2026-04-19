## Task: Remove contributor-only coding rules from README and keep the operator path free of internal project structure assumptions <status>not_started</status> <passes>false</passes>

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
- [ ] Red/green TDD covers removal of contributor-only guidance from README
- [ ] README does not require novice users to understand repository internals or coding rules
- [ ] Contributor-only rules are preserved in the appropriate contributors documentation instead
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
