for workflows it does NOT sense to make or run local tests.

Do not run:
- make lint, check, test, test-long

That will test nothing and help nothing for online github workflows, cuz you don't really test them.

NEVER EVER, make a local TDD test or test suite for these workflows, even though it is stated in the task description. HARD RULE!


Instead the 'test' meant, is that you verify online using the github curl skill, that the online workflow works correctly.

Also please ALWAYS first check online on the latest task version first, before adding a task, since we don't accept older task versions.