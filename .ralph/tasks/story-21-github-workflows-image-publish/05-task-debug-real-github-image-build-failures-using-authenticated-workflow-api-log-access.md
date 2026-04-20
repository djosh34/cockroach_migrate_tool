## Task: Debug real GitHub image-build failures using authenticated workflow API log access until the published runs succeed <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Add an explicit task for debugging failing GitHub Actions image builds against the real hosted workflow runs instead of reasoning from local guesses alone. The higher order goal is to make the workflow story evidence-based: the image pipeline is only fixed when the hosted GitHub runs and logs show it is fixed.

In scope:
- inspect workflow runs, jobs, and logs for failing image builds through authenticated GitHub API access
- use the local authenticated GitHub API curl wrapper/skill instead of exposing tokens or relying on unauthenticated guesses
- iterate on workflow/task fixes until the hosted image-build runs succeed for the three-image split
- capture the real causes of failure found in hosted CI, including architecture-specific failures
- inspect real hosted logs for secret-masking/redaction behavior as part of workflow verification
- verify that trusted-secret usage is gated to the intended `master` push path only

Out of scope:
- broad repository triage unrelated to image build and publish workflows
- hiding or swallowing workflow failures

Decisions already made:
- image builds do not work at all right now
- the fix must be validated against real GitHub workflow logs/results
- the authenticated GitHub API curl wrapper/skill is the intended path for inspecting workflow runs safely
- both `arm64` and `amd64` image paths matter during this debugging work
- real log inspection should include checking that redaction is functioning correctly
- secret-gating failures are workflow bugs and must be treated as real failures

</description>


<acceptance_criteria>
- [x] Red/green TDD covers the local logic around workflow/result expectations where practical
- [x] Real hosted GitHub workflow runs and logs have been inspected through authenticated API access until the image builds succeed
- [x] The task records or reflects the actual CI failure modes fixed, including any arch-specific publish failures and any secret-gating or redaction failures found
</acceptance_criteria>
