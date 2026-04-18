Never ignore the linter, the linters are there with good reason.
Skipping tests is one of the worst things you can do, giving extremely false confidence. Never skip a test, if something is missing in order to test -> fail.

We STRONGLY advice against using 'mut', and MOST of the time it can be replaced by pure and functional patterns.
When creating new enums/structs, always first verifiably search the codebase for overlaps and aim to alter that existing state in order to reuse it instead of creating a new one.

Never swallow/ignore any errors. That is a huge anti-pattern, and must be reported as add-bug task.

This is greenfield project with 0 users. 
We don't have legacy at all. If you find any legacy code/docs, remove it.
No backwards compatibility allowed!
You are encouraged to make large refactors and schema changes
There are no 'versions', no v2/v1 configs, only the current version
Always aim for ultra simple when possible. Can I reuse the same types? Can I merge types with multiple uses?
Never overengineer, overcomplicate, built ANYTHING for 'future-use', you are always wrong in predicting what's needed. 
Only built exactly what you need, when you need something new, try to reuse existing parts and changing them instead.
NEVER CREATE FUNCTIONALITY, UNTIL YOU ACTUALLY NEED IT, no pre-created functions/stuff/fluff/fields/whatever

Never reinvent the wheel. use proven existing packages: e.g. sqlx for connecting, some http framework for rest-api, rustls

Most fields e.g. instance_name, settings and paths are in cfg. Never supply that via func args, instead refer to own &cfg directly.
&Config then lives inside struct, and &self is used to access that.
You create a ctx thing once, by supplying &cfg, this way other code never has to re-supply those

We use thiserror
Never run `cargo test` in this repo.
If you need a focused local test while developing, use `cargo nextest ...`, not `cargo test`.

## Cross application applicable learnings
- Do not run `make test` and `make test-long` concurrently; the HA real-resource nextest coverage can interfere across shared resources and produce misleading failures.
- If `cargo nextest` fails with a cargo archive/object-file error during build, rerun with `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 RUSTFLAGS='-Ccodegen-units=1'`.
