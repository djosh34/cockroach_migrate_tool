# Friction Log

## F1: Missing prerequisite section

Severity: critical

Evidence:

- `cockroach version` failed with `command not found`
- `molt --version` failed with `command not found`

Why this matters:

The novice has to reverse-engineer required tooling from runtime errors. The README should state required host tools before any workflow begins.

## F2: Source bootstrap example is not runnable as written

Severity: critical

Evidence:

```text
config: failed to read webhook CA certificate `.../config/ca.crt`
```

Why this matters:

The first source-bootstrap command fails because the example config references a certificate path the README never tells the user to create or obtain.

## F3: Rendered bootstrap script has a second hidden prerequisite

Severity: critical

Evidence:

```text
.../cockroach-bootstrap.sh: line 7: cockroach: command not found
```

Why this matters:

Even after recovering from the CA-file issue, the next documented step fails because the rendered script assumes the external `cockroach` CLI. That dependency is not surfaced where the source flow is described.

## F4: README does not clearly frame `*.example.internal` values

Severity: high

Evidence:

- `pg_dump` failed with `could not translate host name "pg-a.example.internal" to address`
- `runner run` failed with `Name does not resolve` for the same host

Why this matters:

A novice can tell the hosts look illustrative, but the README does not clearly say whether this is a non-runnable example, a template to edit, or something expected to exist already. That ambiguity blocks trust.

## F5: Schema export failures cascade into misleading later errors

Severity: high

Evidence:

- failed export commands left zero-byte `crdb_schema.txt` and `pg_schema.sql`
- `compare-schema` reported:

```text
schema compare mismatch:
- missing table on cockroach: public.customers
- missing table on cockroach: public.orders
```

- `render-helper-plan` reported:

```text
helper plan: schema compare mismatch:
- missing table on cockroach: public.customers
- missing table on cockroach: public.orders
```

Why this matters:

The later commands do not help the operator recover. The real issue was that the export step failed upstream, not that the databases truly disagreed semantically.

## F6: Top-level README hides the most runnable local path

Severity: high

Evidence:

- top-level README says `investigations/` should not be needed
- `investigations/cockroach-webhook-cdc/README.md` contains the concrete rerun commands:

```bash
./scripts/run.sh
./scripts/run-molt-verify.sh
```

Why this matters:

This is a documentation boundary problem. The easiest local path appears to live in a side directory that the main README explicitly tells the user to ignore.

## F7: Runtime startup needs a free host port

Severity: medium

Evidence:

```text
Bind for 0.0.0.0:8443 failed: port is already allocated
```

Why this matters:

This is a normal container concern, not a design defect by itself. Still, it is one more place where a novice learns by failing unless the README mentions that the published port must be free or adjustable.

## F8: Some generated docs are meaningfully better than the top-level onboarding

Severity: medium positive signal

Evidence:

- `validate-config` emitted a compact, useful success summary
- `render-postgres-setup` succeeded and generated readable README artifacts

Why this matters:

The project already has good operator-facing wording in some generated surfaces. That makes the top-level README’s rough edges more fixable: the clarity exists, but it is not yet concentrated where the novice starts.
