# Novice-User Manual Experience Summary

## Verdict

The current top-level `README.md` is not novice-usable as a self-sufficient quick start.

The destination-side container flow is partially usable: `docker build`, TLS generation, `validate-config`, and `render-postgres-setup` all worked with the documented interface and gave reasonably clear output. The source-side bootstrap flow failed on the very first command because the example config references a CA file that the README never tells the user to create or obtain. After recovering from that gap, the generated script failed immediately because the `cockroach` CLI is required but never listed as a prerequisite.

The workflow also mixes two very different modes without naming them clearly:

- an operator-facing contract written around example infrastructure such as `*.example.internal`
- a real local runnable lab hidden under `investigations/cockroach-webhook-cdc/README.md`

That split is the biggest novice trap. The top-level README says the user should not need to inspect `investigations/`, but the actual local rerun instructions live there.

## README Sufficiency

README alone was not sufficient.

Where it failed:

- It does not begin with a prerequisite section, so the novice must discover binary requirements by trial and error. In this environment, `cargo`, `docker`, `openssl`, and `pg_dump` existed, but `cockroach` and `molt` did not.
- The source bootstrap example config includes `webhook.ca_cert_path: ca.crt`, but the source quick start never explains how to create or obtain that certificate.
- The source quick start does not state that the rendered script depends on the external `cockroach` CLI.
- The schema export step assumes access to live CockroachDB and PostgreSQL instances, but the README examples use placeholder hosts without clearly framing them as non-runnable examples.
- Later schema-aware commands do not help the user recover when export steps failed and produced empty files.
- The runtime step assumes host port `8443` is available. That is a normal operational assumption, but it is another thing the user learns only by failing.

One explicit extra lookup was required after the README path stalled:

- `investigations/cockroach-webhook-cdc/README.md`

That file contains a materially simpler local rerun path:

```bash
./scripts/run.sh
./scripts/run-molt-verify.sh
```

For a novice trying to get a real local end-to-end feel, hiding that path outside the top-level README is a major documentation boundary problem.

## Highest-Friction Findings

1. The very first source-bootstrap command is not runnable from the README alone because the example config references a missing CA certificate file.
2. The next source-bootstrap step depends on the `cockroach` CLI, which is not called out anywhere up front.
3. The schema export stage depends on live source and destination infrastructure, but the README presents example hosts without clearly saying whether they are placeholders, required prerequisites, or something already available locally.
4. Failed export steps leave zero-byte schema files behind, and `compare-schema` / `render-helper-plan` respond with table mismatches instead of pointing the user back to the failed export step.
5. The actual local rerun path appears to live in `investigations/`, even though the README explicitly says a novice should not need that directory.

## What Worked Well

- The top-level README does communicate the high-level split between source bootstrap and destination runtime.
- `docker build -t cockroach-migrate-runner .` worked cleanly.
- `validate-config` returned a concise and helpful success message.
- `render-postgres-setup` produced concrete artifacts and generated additional README files that were easy to understand.
- `runner run --config /config/runner.yml` emitted a clear destination connection failure once container networking was no longer the blocker.

## Overall Assessment

The operator contract is promising for someone who already knows the migration shape, but it is too assumption-heavy for a genuine novice. The destination-side UX is close to credible. The source-side UX is not. The README currently reads more like an implementation contract for an informed operator than a beginner-oriented quick start.
