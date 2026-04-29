# Plan: Add Helm And Raw Kubernetes Manifests For Verify-Container

## References

- Task:
  - `.ralph/tasks/story-36-verify-container-k8s-manifests/01-task-add-helm-and-raw-k8s-manifests-for-verify-container.md`
- Required skills for this planning turn:
  - `.agents/skills/improve-code-boundaries/SKILL.md`
  - `.agents/skills/tdd/SKILL.md`
- Current verify-container public contract sampled during planning:
  - `docs/operator-guide/verify-service.md`
  - `openapi/verify-service.yaml`
  - `artifacts/compose/verify.compose.yml`
- Existing repo gates that still apply on the execution turn:
  - `Makefile`
  - `flake.nix`

## Planning Assumptions

- This turn started with no `<plan>` pointer and no execution marker for the task, so it must end after writing this plan and updating the task file.
- The task-level TDD exception applies because this is Kubernetes/Helm/manual-verification work, not application code.
  - Do not add brittle Rust, Go, or shell tests that merely assert YAML strings.
  - Still use the TDD mindset during execution:
    - one real verification slice at a time
    - fail first with an honest command or cluster apply
    - make only the minimum change to pass that slice
- Never run `cargo`.
  - Use the repo's existing Nix-backed `make` and `nix` surfaces where validation is required.
- The verify-container is a long-running HTTP service, not a batch job.
  - The Kubernetes workload shape should therefore be `Deployment` plus `Service` unless repository evidence during execution proves otherwise.
- No backwards compatibility is allowed.
  - Do not preserve stale deploy layout, naming, or config shapes if a cleaner single deployment surface is possible.
- If execution proves the actual verify image contract, command shape, or TLS files differ materially from the sampled docs, switch this file back to `TO BE VERIFIED` and stop immediately instead of forcing a wrong manifest contract.

## Approval And Verification Priorities

- Highest-priority behaviors to prove during execution:
  - the verify-container can run in Kubernetes using the real config-file contract
  - listener TLS is issued by cert-manager and mounted into the pod correctly
  - externally hosted CockroachDB and PostgreSQL endpoints can be configured without in-cluster database stand-ins
  - raw manifests and Helm install produce the same effective runtime behavior
  - manual verification records exact commands and honest readiness/completion evidence for each surface independently
- Lower-priority concerns:
  - extra templating flexibility beyond what is needed for honest operator examples
  - generic chart abstractions that weaken raw-vs-Helm equivalence

## Current State Summary

- The current operator-facing verify deployment surface is Docker/Compose oriented.
  - `artifacts/compose/verify.compose.yml` mounts one config file plus multiple individual cert files.
  - That surface already implies the important runtime boundary:
    - config file at `/config/verify-service.yml`
    - listener/server certs under `/config/certs/`
    - database CA and client cert material also under `/config/certs/`
- The verify-service public contract in `docs/operator-guide/verify-service.md` shows:
  - command shape: `verify-service run --config /config/verify-service.yml`
  - health surface: `/metrics`
  - no `/healthz`
  - listener TLS and optional listener mTLS
  - source and destination TLS material configured by file path
- There is no current repo-local Kubernetes deployment surface for verify-container.
- Because the service may run HTTPS with optional client-auth and has no dedicated health endpoint, probe design cannot assume a plain unauthenticated HTTP GET.

## Improve-Code-Boundaries Focus

- Primary boundary smell:
  - the current deployable operator surface is implicitly spread across docs and Compose-specific file mounts
  - adding raw YAML and Helm independently could duplicate config paths, Secret names, labels, and cert wiring in two places
- Boundary to flatten during execution:
  - define one canonical Kubernetes deployment contract for verify-container:
    - config file mount path
    - Secret names and keys for cert material
    - ConfigMap layout
    - Deployment container args
    - Service port naming
    - labels/selectors
  - render that same contract in:
    - raw manifests
    - Helm templates
- Smells to actively avoid:
  - raw and Helm describing different cert/key names
  - raw and Helm diverging on ports, args, probe policy, or mount layout
  - inventing a second operator config format just for Kubernetes
  - scattering common metadata across many unrelated YAML files without one obvious shape
- Working boundary decision:
  - create a single repo-local Kubernetes surface such as `deploy/kubernetes/verify-container/`
  - inside it keep:
    - `raw/` for plain manifests
    - `helm/verify-container/` for the chart
    - one concise README for operator usage and manual verification notes
  - keep raw and Helm semantically aligned by sharing the same chosen names, file keys, and resource model

## Intended Kubernetes Contract

- Workload:
  - `Deployment` with one verify-service container
  - `Service` exposing the listener port
- Configuration boundary:
  - one `ConfigMap` containing `verify-service.yml`
  - one or more `Secret` objects for:
    - listener TLS material issued by cert-manager
    - database CA/client cert/key material
    - credential values that must not live in the `ConfigMap`
- Cert-manager boundary:
  - include an `Issuer` or `ClusterIssuer` wiring example for local verification
  - include a `Certificate` resource that writes the listener server certificate Secret consumed by the pod
- Pod mount layout:
  - preserve the current documented file contract under `/config`
  - config file at `/config/verify-service.yml`
  - cert and secret files mounted under stable paths used by the config file
- Networking and probes:
  - use the real listener port from the verify-service contract, not a guessed one
  - prefer `tcpSocket` readiness/liveness probes unless execution proves an HTTP probe is safe for both TLS and optional mTLS modes
  - avoid pretending `/healthz` exists
- Runtime image:
  - make image repository/tag configurable
  - record the exact image reference used during manual verification

## Design Decisions To Confirm During Execution

- Verify the exact container image name published by the repo and the current command/entrypoint behavior.
  - The sampled docs use `verify-image`, but execution must confirm the authoritative current image contract before writing the final manifests.
- Verify whether the listener should stay HTTPS-only in the Kubernetes example or whether an HTTP local-dev mode is still part of the current desired contract.
  - The task explicitly requires cert-manager TLS, so the operator example should be TLS-first unless the repo proves otherwise.
- Verify whether listener mTLS should be included in the example or whether one-way TLS is the minimum honest example.
  - Database-side TLS material still needs to be supported either way.
- Verify whether the raw-table endpoint needs to be exposed via `Service` comments/notes or whether the service example only needs `/metrics` and `/jobs`.
- Verify whether a `ServiceAccount` is necessary.
  - If the pod does not call the Kubernetes API, omit unnecessary RBAC.

## Nix And Local Cluster Decision Gate

- Execution must first consider a Nix-based local Kubernetes setup.
  - Sample honest attempt path:
    - inspect whether `nix shell nixpkgs#kind nixpkgs#kubectl nixpkgs#helm` is enough without flake surgery
    - assess whether cert-manager installation and cluster lifecycle stay simple from that path
- If that path requires broad unrelated flake changes, hidden host prerequisites, or fragile bootstrapping, reject it immediately.
- If Nix is rejected, execution must record:
  - that Nix was tried first
  - why it was deemed too complex for this task
  - which non-Nix cluster path was used instead
- Preferred non-Nix fallback order:
  - `kind`
  - `k3d`
  - `minikube`
  - another already-installed local cluster path only if it is clearly lower risk on this machine

## Manual Verification Strategy

- External dependency requirement:
  - execution must confirm the availability of real external CockroachDB and PostgreSQL endpoints before claiming success
  - if unavailable, the task must remain failed with the exact blocker recorded
- Raw-manifest verification must be independent:
  - create or reuse a local cluster
  - install cert-manager
  - apply raw manifests
  - wait for `Certificate`, `Secret`, `Deployment`, `Pod`, and `Service` readiness
  - prove the verify pod is actually running with the intended config and cert files
  - exercise TLS-authenticated connectivity behavior from inside Kubernetes to the external databases
- Helm verification must be independent:
  - clean the raw installation out of the cluster or use a separate namespace
  - install the chart with values matching the raw-manifest example
  - wait for the same readiness conditions
  - prove the same connectivity and TLS behavior again
- Evidence to capture in task notes/README during execution:
  - cluster tool choice
  - cert-manager install command
  - raw apply commands
  - Helm install command
  - readiness commands and their successful output summary
  - exact image reference used
  - proof method for database connectivity/TLS behavior

## Execution Slices

### Slice 1: Verify The Real Container Contract Before Writing YAML

- RED:
  - inspect the current verify image/runtime contract and let any image-name or command-shape ambiguity surface honestly
- GREEN:
  - pin the actual container args, port, config path, and TLS file paths from authoritative repo sources
- REFACTOR:
  - remove any stale or conflicting deployment assumptions from the new Kubernetes surface instead of preserving both
- Stop condition:
  - if authoritative sources disagree in a way that changes the deployment model, switch back to `TO BE VERIFIED`

### Slice 2: Establish One Canonical Kubernetes Layout

- RED:
  - sketch the raw and Helm resource set and identify the first place where duplication or naming drift would appear
- GREEN:
  - create the deployment directory structure and choose one stable naming/mount/layout contract
- REFACTOR:
  - keep the shared conceptual contract obvious in file layout and resource names
- Verification:
  - render the Helm chart locally and compare its effective structure against the raw manifests for semantic parity

### Slice 3: Raw Manifests Tracer Bullet

- RED:
  - apply the first raw manifest set to a local cluster and let the first honest failure happen
  - likely first failures:
    - missing namespace/resource ordering
    - bad cert-manager wiring
    - wrong mount paths
    - wrong listener port or command args
- GREEN:
  - add the minimum raw resources needed to get the verify pod ready
- REFACTOR:
  - keep Secret and ConfigMap boundaries clean; do not put sensitive values into the `ConfigMap`
- Verification:
  - pod becomes ready
  - service exposes the correct port
  - cert-manager issues the listener certificate Secret

### Slice 4: Prove External TLS Connectivity From Kubernetes

- RED:
  - exercise the verify pod against the real external CockroachDB/PostgreSQL endpoints and let the first connection or TLS error fail honestly
- GREEN:
  - fix config file content, Secret keys, mount paths, or values injection until the pod can perform the intended connectivity checks
- REFACTOR:
  - keep all connection-sensitive data in Secrets and stable file paths
- Verification:
  - capture logs, API behavior, or pod-exec evidence that proves TLS-authenticated database connectivity is being exercised
- Stop condition:
  - if no real external databases are available, record the blocker and stop with the task still failing

### Slice 5: Helm Surface With Raw-Parity

- RED:
  - install the first chart version independently and let any drift from the raw contract fail honestly
- GREEN:
  - template the chart so it produces the same Deployment/Service/ConfigMap/Secret/cert-manager behavior as the raw manifests
- REFACTOR:
  - remove unnecessary chart abstraction if it obscures parity with raw YAML
- Verification:
  - `helm template` remains semantically aligned with raw resources
  - independent Helm install reaches the same ready state and connectivity proof

### Slice 6: Operator Notes And Final Gates

- RED:
  - run repo gates and let any lint/check/test failure surface honestly
- GREEN:
  - fix the repo until:
    - `make check`
    - `make lint`
    - `make test`
    all pass
- REFACTOR:
  - run a final improve-code-boundaries review on the deployment surface
  - if raw and Helm still duplicate muddy naming or conflicting contract details, clean that up before closing
- Documentation boundary:
  - add concise operator-facing usage notes only for:
    - raw apply
    - Helm install
    - values/secrets expected from the operator
    - exact manual verification commands/evidence

## Final Verification Checklist For The Execution Turn

- [ ] A clear repo-local Kubernetes deployment surface exists for verify-container
- [ ] Raw manifests exist and use `ConfigMap` plus `Secret` boundaries correctly
- [ ] A Helm chart exists and matches the raw-manifest behavior
- [ ] cert-manager resources issue the listener TLS Secret used by the verify pod
- [ ] The final config file paths match the actual verify-service contract
- [ ] No in-cluster CockroachDB or PostgreSQL deployment was added
- [ ] External CockroachDB and PostgreSQL endpoints and credentials are configurable through the Kubernetes surface
- [ ] A Nix-based local cluster path was considered first and the result was recorded honestly
- [ ] Raw manifests were applied and verified independently on a local cluster
- [ ] Helm install was applied and verified independently on a local cluster
- [ ] TLS-authenticated connectivity behavior to the external databases was proven honestly
- [ ] `make check` passed
- [ ] `make lint` passed
- [ ] `make test` passed
- [ ] The resulting deployment surface is not muddy under the improve-code-boundaries review

NOW EXECUTE
