# Recommendations

## Prioritized Simplifications

### 1. Add an explicit prerequisite section at the top of `README.md`

Priority: highest

State the required host tools and when they are needed:

- `cargo`
- `docker`
- `openssl`
- `cockroach`
- `pg_dump`
- `molt`

Also state clearly whether a local CockroachDB source and PostgreSQL destination must already exist before starting the quick start.

Why this is first:

It removes the very first layer of trial-and-error and makes every later failure more honest.

### 2. Fix the source bootstrap quick start so the first command is actually runnable

Priority: highest

Choose one:

- document exactly how to create the CA certificate before the example config uses it
- or make the source bootstrap example not require an external CA file for the first render step

Why this is first:

The current first command fails immediately. A novice loses trust before seeing any project value.

### 3. Surface the `cockroach` CLI dependency where the source flow is introduced

Priority: highest

The source section should explicitly say the rendered script shells out to `cockroach sql` and therefore requires the CockroachDB CLI on the host.

Why this is first:

The current flow has two hidden blockers in a row. That makes the source-side experience feel fragile.

### 4. Split the README into two clearly labeled paths

Priority: high

Create a visible distinction between:

- `Local lab / trial run`
- `Real operator contract / production-shaped example`

If the local rerun path is currently `investigations/cockroach-webhook-cdc/README.md`, either promote that flow into the main README or link it explicitly near the top.

Why this matters:

The current document mixes placeholder infrastructure and real operational commands without telling the novice which mode they are in.

### 5. Mark example hostnames and config values as placeholders

Priority: high

Add brief callouts like:

- "Replace `crdb.example.internal` with your CockroachDB host"
- "Replace `pg-a.example.internal` with your PostgreSQL host"
- "This README does not provision those databases for you"

Why this matters:

A novice should not have to infer whether example hosts are illustrative or expected to resolve.

### 6. Make schema-aware commands point back to failed exports

Priority: high

When schema files are empty or obviously malformed, `compare-schema` and `render-helper-plan` should fail with messages that say the export step likely failed and the operator should regenerate the schema inputs.

Why this matters:

The current "missing table on cockroach" output is technically true but operationally misleading in the zero-byte-file case.

### 7. Reuse the generated setup README style in top-level onboarding

Priority: medium

The generated PostgreSQL setup artifacts are concise and direct. Their tone is stronger than the top-level README in the places where the novice most needs certainty.

Why this matters:

This is an improve-code-boundaries style docs fix: move operator knowledge to the correct boundary instead of scattering essential setup knowledge between README, generated docs, and investigations.

### 8. Mention the host-port assumption on the runtime example

Priority: medium

Add one short note near `-p 8443:8443`:

- "`8443` must be free on the host, or change the published host port"

Why this matters:

This is not the main blocker, but it is an avoidable stumble during first contact.
