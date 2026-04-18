# Requirements And Constraints

## Functional Requirements

- Migrate one or more CockroachDB databases to one or more PostgreSQL databases.
- Support a one-time migration with initial load and live catch-up until cutover.
- Keep the destination runner capable of continuous operation indefinitely until the operator chooses cutover.
- Allow excluding some tables from validation and transfer.
- Validate that the destination schema is sufficiently compatible before transfer starts.
- Apply inserts, updates, and deletes.
- Track progress in PostgreSQL so the system can resume after failure.
- Provide a source-side command that can be run non-interactively in a pipeline.
- Provide a destination-side binary that runs in a container on the PostgreSQL host.
- Verify the final migrated result with MOLT verify.
- Store migration progress and internal transfer state inside each destination database.
- Support a final API-level write freeze and drain-based cutover.
- Return webhook success only after the webhook payload has been durably persisted into PostgreSQL migration tables.
- For each migrated real table, create a corresponding helper shadow table in `_cockroach_migration_tool`.
- Support one single destination container that exposes the webhook HTTPS endpoint and connects to PostgreSQL with a scoped role only.
- Support multiple source databases and multiple destination databases controlled by that one destination container.

## Non-Functional Requirements

- No silent error swallowing.
- No skipped tests when implementing the real system.
- Design for rerunability and operational clarity.
- Work with scoped PostgreSQL roles rather than assuming superuser access.
- Preserve correctness with FK and PK constraints enabled on the destination.
- Keep the design intentionally simple and avoid unnecessary generic machinery.
- Use `thiserror` for application error types.
- Prefer established libraries for HTTP, TLS, config, CLI, and database integration instead of reinventing those pieces.
- Do not introduce libraries into implementation tasks before they are actually needed by the scoped story.
- End-to-end tests must be real end-to-end with no fake migrations, no shortcuts, and no hidden side-channel control of the source after CDC setup.
- The quick-start path must be fully understandable from the README alone by a novice user.

## Explicitly Out Of Scope

- Automatic generation of a compatible PostgreSQL schema from CockroachDB schema.
- Full implementation of the final migration product in this phase.
- Final webhook receiver implementation in this phase.

## Design Pressure Points

- CockroachDB webhook CDC is at-least-once, so duplicate delivery must be assumed.
- Multi-table ordering and FK dependencies are not automatically safe for PostgreSQL apply.
- The operator may not have direct interactive access to the production CockroachDB cluster.
- The destination role is constrained to specific PostgreSQL databases.
- The migration is one-time, but failure recovery still matters because first-run issues are likely.
- PostgreSQL control state must live per destination database, not in a separate central control database.
- Shadow-table ingest and real-table reconcile are intentionally separate steps.
- After CDC setup is completed, tests must not rely on any further source-side shell commands, scripts, or direct database intervention.
