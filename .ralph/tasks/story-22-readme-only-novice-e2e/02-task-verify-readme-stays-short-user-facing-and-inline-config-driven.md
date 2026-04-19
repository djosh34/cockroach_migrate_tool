## Task: Verify the README stays short, user-facing, and driven by inline config examples instead of project philosophy <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a README-content verification task that enforces a short, direct, operator-only guide with inline config examples and minimal explanatory noise. The higher order goal is to stop the README from drifting into philosophy, project-structure commentary, or contributor-facing material that makes novice operation harder.

In scope:
- verify the README talks only to the user/operator for the supported image flows
- verify the README stays short and to the point
- verify the README does not include project philosophy, “why this matters” sections, or contributor/process guidance
- verify contributor-only material is explicitly absent from the README rather than merely documented elsewhere
- verify the README includes fenced code blocks for the configs the user actually needs
- verify the README explains required and optional args in simple list form
- verify the README starts with a simple example and adds extra args only where needed
- verify the README fully covers runner, SQL-emitter, and verify-image usage without sending the user elsewhere

Out of scope:
- rewriting contributor documentation beyond what is required to prove it is not leaking into the README

Decisions already made:
- the README should be a simple short guide
- inline fenced config text is required
- explanations should be list-based and practical
- the guide should start simple and grow only when extra arguments are genuinely needed
- contributor/process guidance must be verified as absent from the README, not treated as somebody else's concern

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers README-content requirements for operator focus, brevity, inline config examples, and simple argument explanation
- [ ] The task fails if the README contains project philosophy, “why this matters”, contributor rules, or repo-structure guidance in the supported user path
- [ ] The task proves contributor-only material is absent from the README rather than indirectly tolerated
- [ ] The task proves the README begins with a simple working example and introduces extra arguments only when needed
- [ ] The task proves the README includes the exact inline config and Compose text needed for the supported image flows
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
