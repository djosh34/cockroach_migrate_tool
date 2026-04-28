## Smell Set: 2026-04-28-runner-config-runtime-boundary <status>completed</status> <passes>true</passes>

Please refer to skill 'improve-code-boundaries' to see what smells there are.

Inside dirs:
- `crates/runner/src`

Solve each smell:

---
- [x] Smell 3, Wrong Place-ism
Config loading, destination validation, SQL name types, validated schema types, and startup planning lived in `runner`, even though that crate also owned webhook serving and reconcile runtime startup. That made the runtime crate act as a courier for lightweight config knowledge and forced validation-oriented code to share the runtime dependency surface.

code:
`crates/runner/src/lib.rs` mixed `LoadedRunnerConfig`, `RunnerStartupPlan`, deep validation, and runtime startup in one entrypoint.
`crates/runner/src/config/mod.rs`
`crates/runner/src/config/parser.rs`
`crates/runner/src/destination_catalog.rs`
`crates/runner/src/sql_name.rs`
`crates/runner/src/validated_schema.rs`
The config/startup boundary has now been moved into `crates/runner-config`.

---
- [x] Smell 6, mixed-responsibilities
`runner` simultaneously owned config-only validation behavior and long-running server/bootstrap behavior. That was a mixed responsibility boundary at the crate level, not just inside one function.

code:
`crates/runner/src/lib.rs`
`crates/runner/src/runtime_plan.rs`
The split now makes `runner-config` own validation/startup-plan responsibilities and `runner` own runtime bootstrap, webhook serving, and reconcile execution.
