## Bug: Remove dumb structure and file-content tests <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
The test suite contains many dumb tests that do not verify product behavior and instead lock down implementation structure, adjacent file contents, documentation phrasing, Dockerfile layout, workflow/compose YAML details, source-code markers, helper names, or exact strings in other files. These tests hinder development because they fail on harmless refactors and file reorganizations while providing little or no useful regression signal.

A dumb test is any test that does not exercise actual behavior through the product boundary. This includes, but is not limited to:

- Tests that read another source, test, workflow, Dockerfile, compose, README, docs, OpenAPI, or config artifact and assert `contains`, `!contains`, marker ordering, word count, headings, helper names, exact snippets, approved file lists, or other textual structure.
- Tests that verify how code is organized, which helper owns a behavior, which file calls Docker, which function name exists, whether an old marker string is absent, or whether an implementation uses a particular source boundary.
- Tests that check adjacent files instead of running the code path they claim to protect.
- Tests that enforce Dockerfile/cache-layer implementation details instead of proving that the image builds, starts, and behaves correctly.
- Tests that enforce documentation wording or README section shape instead of manually verifying documentation when documentation changes.
- Any similar test not explicitly listed here that asserts structure/text/layout rather than behavior.

Detected examples include the OpenAPI content contract tests, the runner E2E integrity contract tests that scan source/test files, README and TLS reference documentation tests, Dockerfile/cache-layer structure tests, compose artifact structure tests, and CLI/image tests that only assert brittle help/documentation/output marker lists. The implementation must remove all such dumb tests, including matching tests beyond this detected list.

Do not replace these dumb tests with new text-scanning tests. If a removed test was trying to protect real behavior, replace it with a behavioral test at the product boundary, such as running the CLI, starting the service, making HTTP requests, building/running the image, or exercising the database flow. For non-code artifacts such as Dockerfiles, workflows, and documentation, do not add Rust/Go text-assert tests; use manual verification appropriate to the artifact instead.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD only when replacing a removed dumb test with a real behavioral code test. Make one behavioral test red, then make that one test green before continuing.

Do not use TDD to preserve documentation, Dockerfile, workflow, compose-file, or other file/text structure assertions. For those removals, delete the dumb tests and manually verify the artifact through the real tool or runtime boundary instead.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [x] Removed every dumb test that asserts source/test/helper structure, marker strings, documentation wording, Dockerfile/cache-layer layout, workflow/compose/OpenAPI file contents, or adjacent file text instead of behavior
- [x] Audited the whole tracked test suite for additional dumb tests beyond the initially detected examples and removed those too
- [x] Did not add replacement tests that scan files for strings, marker ordering, helper names, approved call-site lists, headings, word counts, or similar structural/textual details
- [x] Where a removed dumb test protected a real product behavior, replaced it with a behavioral test through the real boundary
- [x] For documentation, Dockerfile, compose, workflow, and OpenAPI changes, used manual/tool verification instead of adding text-assert tests
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this cleanup changes ultra-long test selection or support: `make test-long` — passes cleanly (not applicable; long-lane support was unchanged)
</acceptance_criteria>
