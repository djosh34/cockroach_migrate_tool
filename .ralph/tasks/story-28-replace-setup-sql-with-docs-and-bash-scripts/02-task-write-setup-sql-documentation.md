## Task: Write clear, explanatory docs in `./docs/setup_sql/` covering what SQL must be run on each database and why <status>not_started</status> <passes>false</passes>

<description>
**Goal:** Create comprehensive, operator-friendly documentation at `./docs/setup_sql/` that explains exactly what SQL statements need to be executed on the source CockroachDB cluster and on the destination PostgreSQL cluster, with per-database/per-table template examples using a Jinja2-like syntax, and clear guidance on how to determine the values to fill in.

The higher-order goal is to replace the opaque Rust binary with transparent, maintainable docs that any operator or developer can read and act on. This is a greenfield project with zero users — no legacy to carry forward.

In scope:
- Create `./docs/setup_sql/` directory structure
- Write `./docs/setup_sql/cockroachdb-source-setup.md` covering:
  - Overview: why these steps are needed (enable rangefeed, capture cursor, create changefeeds)
  - Step-by-step explanation of each SQL statement
  - Per-statement: what it does, on which CockroachDB cluster it must run, prerequisites
  - Jinja2-style template that shows the exact SQL with `{{ variable }}` placeholders
  - Variable reference table: each placeholder (e.g. `{{ webhook_base_url }}`, `{{ ca_cert_base64 }}`, `{{ resolved_interval }}`, `{{ database }}`, `{{ schema }}`, `{{ table }}`, `{{ mapping_id }}`), what it means, how to determine its value, example values
  - Full annotated example for a multi-database, multi-mapping scenario
  - Notes on cursor capture: why you capture once, where to paste the value
- Write `./docs/setup_sql/postgresql-destination-grants.md` covering:
  - Overview: why grants are needed (runtime role needs CONNECT/CREATE on database, USAGE on schema, DML on tables)
  - Step-by-step explanation of each SQL statement
  - Per-statement: what it does, on which PostgreSQL server it must run, prerequisites (database must exist)
  - Jinja2-style template that shows the exact SQL with `{{ variable }}` placeholders
  - Variable reference table: each placeholder (e.g. `{{ database }}`, `{{ runtime_role }}`, `{{ schema }}`, `{{ table }}`), what it means, how to determine its value, example values
  - Full annotated example for a multi-database, multi-mapping scenario
- Write `./docs/setup_sql/index.md` with a table of contents linking both guides and a quick-reference summary of which SQL runs where

The templates should use Jinja2 syntax (`{{ var }}`, `{% for item in items %}`) because the bash scripts (Task 03) will use a tool that understands this syntax. The docs must stand alone — readable by a human who will execute the SQL manually if they choose.

Out of scope:
- Writing the bash scripts themselves (Task 03)
- Modifying any existing docs outside `./docs/setup_sql/`
- Any code changes to the runner or other crates
</description>

<acceptance_criteria>
- [ ] `./docs/setup_sql/index.md` exists with TOC and quick-reference
- [ ] `./docs/setup_sql/cockroachdb-source-setup.md` exists with:
  - [ ] Complete Jinja2 template for the CockroachDB SQL
  - [ ] Variable reference table with meanings and how-to-fill-in for every placeholder
  - [ ] Annotated multi-database multi-mapping example
  - [ ] Cursor capture instructions
- [ ] `./docs/setup_sql/postgresql-destination-grants.md` exists with:
  - [ ] Complete Jinja2 template for the PostgreSQL SQL
  - [ ] Variable reference table with meanings and how-to-fill-in for every placeholder
  - [ ] Annotated multi-database multi-mapping example
- [ ] Every template variable is documented: `{{ webhook_base_url }}`, `{{ ca_cert_base64 }}`, `{{ resolved_interval }}`, `{{ database }}`, `{{ schema }}`, `{{ table }}`, `{{ mapping_id }}`, `{{ runtime_role }}`, `{{ cockroach_url }}`
- [ ] A reader with no prior knowledge can determine what SQL to run where and what values to substitute
- [ ] Manual review: the rendered templates produce valid SQL matching the contract that the runner expects (mapping ingest paths, changefeed options, grant statements)
- [ ] `make check` — passes cleanly (docs-only change, no code impacts)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
