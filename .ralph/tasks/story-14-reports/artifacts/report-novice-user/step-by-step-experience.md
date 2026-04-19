# Step-By-Step Novice Experience

## Trial Setup

I treated `README.md` as the public contract and deliberately avoided reading code. I only looked outside the README after the documented path stopped being self-sufficient, and I recorded that extra lookup explicitly.

I ran the trial from the repository root but kept generated trial inputs and outputs under `.ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/` so the evidence stayed contained.

## Slice 1: README-only orientation

### First impression

The README gives a decent conceptual split:

- source side: render a CockroachDB bootstrap script and run it manually
- destination side: build one `runner` image, validate config, compare schema, render setup artifacts, and then run the runtime

That mental model is understandable. What is missing is a novice-oriented prerequisites section and a clear statement of whether this is:

- a runnable local developer quick start
- an operator contract with placeholder infrastructure

The document uses obviously fake hosts like `crdb.example.internal` and `pg-a.example.internal`, but it does not say that these are placeholders or tell the novice how to substitute them safely.

### Commands used

```bash
cargo --version
docker --version
openssl version
cockroach version
pg_dump --version
molt --version
```

### Observed results

- `cargo`, `docker`, `openssl`, and `pg_dump` existed.
- `cockroach` was missing: `/bin/bash: line 1: cockroach: command not found`
- `molt` was missing: `/bin/bash: line 1: molt: command not found`

### Novice reaction

I had to start building my own prerequisite list from failures. That is the first sign the README is written for someone who already knows the tooling stack.

## Slice 2: Source bootstrap quick start

### README config used

I created the README example config as written:

```yaml
cockroach:
  url: postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require
webhook:
  base_url: https://runner.example.internal:8443
  ca_cert_path: ca.crt
  resolved: 5s
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.orders
  - id: app-b
    source:
      database: demo_b
      tables:
        - public.invoices
```

### First documented command

```bash
cargo run -p source-bootstrap -- render-bootstrap-script --config .ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/config/source-bootstrap.yml > .ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/cockroach-bootstrap.sh
```

### Result

The command failed immediately:

```text
config: failed to read webhook CA certificate `.ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/config/ca.crt`
```

### Pause and investigation

I checked the CLI help:

```bash
cargo run -p source-bootstrap -- render-bootstrap-script --help
```

It only showed `--config`, with no guidance about the missing CA certificate.

To continue the trial, I created a local CA certificate manually:

```bash
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout .ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/config/ca.key \
  -out .ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/config/ca.crt \
  -days 365 \
  -subj "/CN=runner-ca" \
  -addext "basicConstraints=critical,CA:TRUE"
```

### Second render attempt

```bash
cargo run -p source-bootstrap -- render-bootstrap-script --config .ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/config/source-bootstrap.yml > .ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/cockroach-bootstrap.sh
```

This succeeded and produced a real script. The script looked plausible and inlined the CA certificate into the webhook URL.

### Manual execution step

```bash
bash .ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/cockroach-bootstrap.sh
```

### Result

The script failed immediately:

```text
.ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/cockroach-bootstrap.sh: line 7: cockroach: command not found
```

### Novice reaction

This section has the worst ergonomics in the README.

- The first command is not runnable because the config includes an unstated CA prerequisite.
- The second command is not runnable because the rendered script assumes the `cockroach` CLI exists.
- Neither issue is explained in the source quick start itself.

## Slice 3: Docker quick start early path

### TLS generation

I followed the documented TLS step with the trial workspace path adjusted:

```bash
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout .ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/config/certs/server.key \
  -out .ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/config/certs/server.crt \
  -days 365 \
  -subj "/CN=runner.example.internal"
```

This worked without surprises.

### Build the image

```bash
docker build -t cockroach-migrate-runner .
```

This also worked cleanly. The build was cached in this environment, which made it fast, but the command itself behaved as expected.

### Validate config

```bash
docker run --rm \
  -v "$(pwd)/.ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/config:/config:ro" \
  cockroach-migrate-runner \
  validate-config --config /config/runner.yml
```

### Result

```text
config valid: config=/config/runner.yml mappings=1 verify=molt@/work/molt webhook=0.0.0.0:8443 tls=/config/certs/server.crt+/config/certs/server.key
```

### Novice reaction

This is the best part of the current operator UX. The command is short enough, the success message is concise, and it confirms the major config surfaces.

## Slice 4: Schema and setup workflow

### Export CockroachDB schema

```bash
cockroach sql \
  --url "postgresql://root@crdb.example.internal:26257/demo_a?sslmode=require" \
  --execute "SHOW CREATE ALL TABLES;" > .ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/schema/crdb_schema.txt
```

### Result

```text
/bin/bash: line 1: cockroach: command not found
```

The shell still created a zero-byte `crdb_schema.txt` file before failing.

### Export PostgreSQL schema

```bash
pg_dump \
  --schema-only \
  --no-owner \
  --no-privileges \
  --dbname "postgresql://postgres@pg-a.example.internal:5432/app_a?sslmode=require" \
  > .ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/schema/pg_schema.sql
```

### Result

```text
pg_dump: error: could not translate host name "pg-a.example.internal" to address: Name or service not known
```

This also left a zero-byte `pg_schema.sql`.

### Render PostgreSQL setup artifacts

```bash
docker run --rm \
  -v "$(pwd)/.ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/config:/config:ro" \
  -v "$(pwd)/.ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/postgres-setup:/work/postgres-setup" \
  cockroach-migrate-runner \
  render-postgres-setup --config /config/runner.yml --output-dir /work/postgres-setup
```

### Result

```text
postgres setup artifacts written: output=/work/postgres-setup mappings=1
```

Generated files:

- `postgres-setup/README.md`
- `postgres-setup/app-a/README.md`
- `postgres-setup/app-a/grants.sql`

The generated README files were short and helpful. This part of the UX felt clear.

### Compare schemas

```bash
docker run --rm \
  -v "$(pwd)/.ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/config:/config:ro" \
  -v "$(pwd)/.ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/schema:/schema:ro" \
  cockroach-migrate-runner \
  compare-schema \
  --config /config/runner.yml \
  --mapping app-a \
  --cockroach-schema /schema/crdb_schema.txt \
  --postgres-schema /schema/pg_schema.sql
```

### Result

```text
schema compare mismatch:
- missing table on cockroach: public.customers
- missing table on cockroach: public.orders
```

### Render helper plan

```bash
docker run --rm \
  -v "$(pwd)/.ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/config:/config:ro" \
  -v "$(pwd)/.ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/schema:/schema:ro" \
  -v "$(pwd)/.ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/helper-plan:/work/helper-plan" \
  cockroach-migrate-runner \
  render-helper-plan \
  --config /config/runner.yml \
  --mapping app-a \
  --cockroach-schema /schema/crdb_schema.txt \
  --postgres-schema /schema/pg_schema.sql \
  --output-dir /work/helper-plan
```

### Result

```text
helper plan: schema compare mismatch:
- missing table on cockroach: public.customers
- missing table on cockroach: public.orders
```

### Novice reaction

The tool outputs are technically consistent, but not novice-friendly in this failure mode. The real problem was upstream:

- missing `cockroach` CLI
- placeholder PostgreSQL host that does not resolve
- two zero-byte export files

The later commands do not point the user back to those causes.

## Slice 5: Runtime startup and mental model

### First runtime attempt

```bash
timeout 20s docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/.ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/config:/config:ro" \
  cockroach-migrate-runner \
  run --config /config/runner.yml
```

### Result

```text
docker: Error response from daemon: failed to set up container networking: driver failed programming external connectivity on endpoint ...: Bind for 0.0.0.0:8443 failed: port is already allocated
```

I checked running containers:

```bash
docker ps --format '{{.ID}} {{.Ports}} {{.Names}}'
```

This showed another local stack already holding `8443`, `5432`, and CockroachDB ports.

### Second runtime attempt without host port publishing

```bash
timeout 20s docker run --rm \
  -v "$(pwd)/.ralph/tasks/story-14-reports/artifacts/report-novice-user/workdir/config:/config:ro" \
  cockroach-migrate-runner \
  run --config /config/runner.yml
```

### Result

```text
postgres bootstrap: failed to connect mapping `app-a` to `pg-a.example.internal:5432/app_a`: error communicating with database: failed to lookup address information: Name does not resolve
```

### Novice reaction

This is a better error than the schema-compare failure mode. It directly tells the operator what the runtime was trying to connect to and why it failed. The remaining problem is that the README never turns the example hostnames into a clear, runnable local setup.

## Extra Investigation After README Failure

The README claims:

- "You should not need to inspect `crates/`, `tests/`, or `investigations/` to complete this quick start."

Because the documented flow still did not yield a runnable local path, I looked at:

```bash
sed -n '1,260p' investigations/cockroach-webhook-cdc/README.md
```

That file contains a much more concrete rerun path:

```bash
./scripts/run.sh
./scripts/run-molt-verify.sh
```

It also explains that it generates certs, starts CockroachDB and PostgreSQL with Docker, and exercises CDC plus `molt verify`.

### Final novice reaction

This extra lookup confirmed the main documentation boundary problem:

- the top-level README reads like the authoritative quick start
- the most runnable local investigation path is hidden somewhere the README explicitly says I should not need

That is the clearest sign that the current docs are optimized for someone already oriented to the project, not for a genuine first-time operator.
