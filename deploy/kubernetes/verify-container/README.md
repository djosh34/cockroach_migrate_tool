# Verify-Container Kubernetes Surface

This directory provides two semantically equivalent ways to run the `verify-service` container on Kubernetes:

- `raw/` uses plain Kubernetes manifests with `kubectl apply -k`
- `helm/verify-container/` provides the same workload through a Helm chart

Both surfaces keep the same runtime contract:

- config file mounted at `/config/verify-service.yml`
- all certificate material mounted under `/config/certs/`
- credential files mounted under `/config/secrets/`
- listener TLS issued by cert-manager into the `verify-container-listener-tls` Secret
- listener served on `8443`
- health checked with TCP probes because the service exposes `/metrics`, not `/healthz`

## Raw manifests

Create the required local input files first:

```bash
cp deploy/kubernetes/verify-container/raw/config/verify-service.yml.example \
  deploy/kubernetes/verify-container/raw/config/verify-service.yml
cp deploy/kubernetes/verify-container/raw/secrets/source-username.example \
  deploy/kubernetes/verify-container/raw/secrets/source-username
cp deploy/kubernetes/verify-container/raw/secrets/destination-username.example \
  deploy/kubernetes/verify-container/raw/secrets/destination-username
cp deploy/kubernetes/verify-container/raw/secrets/destination-password.example \
  deploy/kubernetes/verify-container/raw/secrets/destination-password
```

Then place the real database TLS files at:

- `deploy/kubernetes/verify-container/raw/secrets/source-ca.crt`
- `deploy/kubernetes/verify-container/raw/secrets/source-client.crt`
- `deploy/kubernetes/verify-container/raw/secrets/source-client.key`
- `deploy/kubernetes/verify-container/raw/secrets/destination-ca.crt`

Set the real image reference in [deployment.yaml](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/deploy/kubernetes/verify-container/raw/deployment.yaml), then apply:

```bash
kubectl apply -k deploy/kubernetes/verify-container/raw
```

## Helm chart

Install the chart with the same config and secret contract:

```bash
helm upgrade --install verify-container \
  deploy/kubernetes/verify-container/helm/verify-container \
  --namespace verify-container \
  --create-namespace \
  --set image.repository=ghcr.io/<owner>/verify-image \
  --set image.tag=<git-sha> \
  --set config.source.host=<cockroach-host> \
  --set config.destination.host=<postgres-host> \
  --set-file secrets.credentials.sourceUsername=deploy/kubernetes/verify-container/raw/secrets/source-username \
  --set-file secrets.credentials.destinationUsername=deploy/kubernetes/verify-container/raw/secrets/destination-username \
  --set-file secrets.credentials.destinationPassword=deploy/kubernetes/verify-container/raw/secrets/destination-password \
  --set-file secrets.tls.sourceCaPem=deploy/kubernetes/verify-container/raw/secrets/source-ca.crt \
  --set-file secrets.tls.sourceClientCertPem=deploy/kubernetes/verify-container/raw/secrets/source-client.crt \
  --set-file secrets.tls.sourceClientKeyPem=deploy/kubernetes/verify-container/raw/secrets/source-client.key \
  --set-file secrets.tls.destinationCaPem=deploy/kubernetes/verify-container/raw/secrets/destination-ca.crt
```

The chart renders the same Secret keys, mount paths, cert-manager issuer chain, Service, and Deployment shape as the raw manifests.

## Local verification

The local verification helper creates external Dockerized CockroachDB and PostgreSQL instances on the `kind` network, seeds matching `appdb.accounts` data, generates the raw input files, and writes a Helm values file for the same endpoints:

```bash
deploy/kubernetes/verify-container/scripts/prepare-local-verification.sh
```

That helper writes:

- `deploy/kubernetes/verify-container/raw/config/verify-service.yml`
- `deploy/kubernetes/verify-container/raw/secrets/*`
- `deploy/kubernetes/verify-container/.local-verification/raw-overlay/kustomization.yaml`
- `deploy/kubernetes/verify-container/helm/verify-container/values.local.yaml`

Use the generated raw overlay so the local image override stays out of the operator-facing base:

```bash
kubectl apply -k deploy/kubernetes/verify-container/.local-verification/raw-overlay
```

Use the generated Helm values file for the same endpoints:

```bash
helm upgrade --install verify-container \
  deploy/kubernetes/verify-container/helm/verify-container \
  --namespace verify-container \
  --create-namespace \
  --values deploy/kubernetes/verify-container/helm/verify-container/values.local.yaml
```

During local verification in this task, the exact image loaded into `kind` is recorded in the task file after the checks pass.
