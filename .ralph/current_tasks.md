# Current Tasks Summary

Generated: Mon Apr 27 11:47:47 PM CEST 2026

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-28-replace-setup-sql-with-docs-and-bash-scripts/02-task-write-setup-sql-documentation.md`

```
## Task: Write clear, explanatory docs in `./docs/setup_sql/` covering what SQL must be run on each database and why <status>not_started</status> <passes>false</passes>

<description>
**Goal:** Create comprehensive, operator-friendly documentation at `./docs/setup_sql/` that explains exactly what SQL statements need to be executed on the source CockroachDB cluster and on the destination PostgreSQL cluster, with per-database/per-table template examples using a Jinja2-like syntax, and clear guidance on how to determine the values to fill in.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-28-replace-setup-sql-with-docs-and-bash-scripts/03-task-create-bash-scripts-to-generate-sql-from-yaml.md`

```
## Task: Create bash scripts that turn a YAML config file into SQL output files for CockroachDB and PostgreSQL separately <status>not_started</status> <passes>false</passes>

<description>
**Goal:** Create two standalone bash scripts that read a simple YAML input file and produce SQL output files — one script for CockroachDB source setup SQL, one script for PostgreSQL destination grants SQL. The scripts must be clearly separated, well-documented, and produce SQL that matches what the docs (Task 02) describe.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-29-optimize-rust-build-story/01-task-drastically-reduce-rust-dependency-footprint-and-compile-time.md`

```
## Task: Drastically Reduce Rust Dependency Footprint And Compile Time <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

