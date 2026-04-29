## Task: Add Helm and raw Kubernetes manifests for running the verify container with cert-manager TLS <status>not_started</status> <passes>false</passes>

<description>
Do not use TDD for this task. This is a Kubernetes manifest, Helm chart, and manual cluster verification task, not application code. Never run `cargo`; use Nix-backed commands only where they are the established repository path, and use non-Nix tooling for the local Kubernetes cluster if Nix-based cluster setup proves too complex.

**Goal:** Add production-shaped Kubernetes examples for the verify container in both Helm and raw Kubernetes manifest form. The higher-order goal is to make the verify container deployable by an operator who wants Kubernetes-native configuration, cert-manager issued certificates, and the same behavior whether they choose Helm or plain `kubectl apply`.

This task must create two equivalent deployment surfaces:
- a Helm chart for the verify container
- raw non-Helm Kubernetes manifest YAML files

Both surfaces must configure the same runtime behavior and must be kept semantically equivalent. Differences are allowed only where Helm templating is inherently needed, such as values substitution, names, labels, and namespace parametrization.

In scope:
- Add a repo-local Kubernetes example location, with clear names chosen during implementation, for example `deploy/kubernetes/verify-container/`.
- Add a Helm chart for `verify-container`.
- Add raw Kubernetes manifests for the same `verify-container` deployment.
- Use cert-manager to issue the TLS certificates needed by the verify container.
- Use a Kubernetes `ConfigMap` for the verify container config.
- Use Kubernetes `Secret` objects only for sensitive values and certificate material.
- Include all ordinary Kubernetes objects needed for a real operator example, such as namespace handling, labels, service account if needed, service, deployment or job shape as appropriate for the current verify HTTP/container contract, probes if supported, resource requests/limits where sensible, and cert-manager `Issuer` or `ClusterIssuer` wiring.
- The manifests must make the verify pod prove TLS-authenticated connectivity behavior from inside Kubernetes, while CockroachDB and PostgreSQL remain outside the Kubernetes cluster.
- CockroachDB and PostgreSQL must not be hosted, installed, bootstrapped, or mocked inside the Kubernetes cluster by this task.
- The configuration must support externally hosted CockroachDB and PostgreSQL endpoints and credentials.
- The task executor must inspect the current verify container contract in the repository rather than guessing ports, paths, config names, command arguments, health endpoints, image names, TLS file locations, or job API behavior.
- If the verify container image reference in the repo is not yet stable, the examples must make the image configurable and document the exact image value used during manual verification.
- Add concise operator-facing README or comments explaining how to apply the raw manifests and how to install the Helm chart, without turning this into a broad docs rewrite.
- Manual verification must actually apply both the raw manifests and the Helm chart to a local Kubernetes cluster and prove they work.

Manual local Kubernetes verification requirement:
- The executor must find the least-friction local Kubernetes approach available on the machine.
- The executor must first consider whether Nix can provide the local Kubernetes setup cleanly.
- If the Nix path looks very complicated, time-consuming, fragile, or requires broad unrelated flake changes, the executor must immediately reject it and use another non-Nix local Kubernetes approach instead.
- If Nix is rejected for local Kubernetes setup, the executor must write in the task execution notes, plan, or this task file that the Nix Kubernetes setup path was tried and deemed overly complex, and must state which non-Nix local Kubernetes approach was used instead.
- Reasonable non-Nix candidates include `kind`, `k3d`, `minikube`, Docker Desktop Kubernetes, or another locally available low-friction cluster, with the final choice based on what is actually available and least risky in this environment.
- The executor must install or use cert-manager in the local cluster for verification.
- The executor must apply the raw manifests and verify the resulting Kubernetes resources reach the expected ready/completed state.
- The executor must install the Helm chart and verify the resulting Kubernetes resources reach the same expected ready/completed state.
- The raw manifests and Helm chart must be tested independently, not treated as one covering the other.
- The executor must prove from the verify pod, or from the verify job/service behavior exposed by the verify pod, that TLS authentication and configured connectivity to externally hosted CockroachDB and PostgreSQL are exercised.
- If real external CockroachDB and PostgreSQL endpoints are unavailable, the task must fail with a clear blocker rather than silently replacing them with in-cluster databases or weaker fake coverage.

Out of scope:
- Hosting CockroachDB in Kubernetes.
- Hosting PostgreSQL in Kubernetes.
- Adding a Kubernetes operator.
- Changing the verify service API or runtime behavior unless the current container contract is impossible to deploy safely; if such a product issue is found, create a separate bug task.
- Broad documentation rewrites unrelated to Kubernetes deployment.
- TDD or Rust test additions for this manifest-only task.

Important project rules:
- Never ignore linter failures.
- Never skip required verification. If required local Kubernetes verification cannot be completed, the task must remain failed with the exact blocker recorded.
- Never swallow or ignore errors. If the task uncovers code that swallows/ignores errors, create a bug task via the `add-bug` skill.
- This is a greenfield project with zero users. Do not preserve legacy Kubernetes examples, docs, flags, config shapes, or backwards compatibility if they conflict with the current desired verify-container contract; remove stale legacy material or create follow-up tasks where removal is too broad.
</description>

<acceptance_criteria>
- [ ] A Helm chart for the verify container exists in a clear repo-local Kubernetes/deploy location.
- [ ] Raw non-Helm Kubernetes manifest YAML files for the verify container exist in a clear repo-local Kubernetes/deploy location.
- [ ] The Helm chart and raw manifests deploy the same verify-container behavior and differ only where Helm templating requires it.
- [ ] The manifests use cert-manager resources to issue the TLS certificates needed by the verify container.
- [ ] The verify container config is supplied through a Kubernetes `ConfigMap`.
- [ ] Sensitive values and certificate material are supplied through Kubernetes `Secret` objects, not through `ConfigMap` data.
- [ ] The manifests include the ordinary supporting Kubernetes resources needed for a usable operator example, such as service account if needed, service, workload object, labels, namespace guidance, resource sizing, and readiness/liveness behavior where supported by the current container contract.
- [ ] CockroachDB is not deployed, installed, bootstrapped, mocked, or hosted in the Kubernetes cluster.
- [ ] PostgreSQL is not deployed, installed, bootstrapped, mocked, or hosted in the Kubernetes cluster.
- [ ] The manifests configure externally hosted CockroachDB and PostgreSQL endpoints and credentials.
- [ ] The executor inspected the current verify container contract in the repository and matched real ports, paths, config names, command arguments, health endpoints, image names, TLS file locations, and API behavior.
- [ ] The executor considered a Nix-based local Kubernetes setup first.
- [ ] If the Nix Kubernetes setup path was rejected, the task execution notes, plan, or this task file explicitly says that Nix was tried and deemed overly complex, and names the non-Nix Kubernetes approach used instead.
- [ ] cert-manager was installed or available in the local Kubernetes cluster used for verification.
- [ ] Manual verification applied the raw Kubernetes manifests to a local cluster and recorded the exact commands and successful readiness/completion evidence.
- [ ] Manual verification installed the Helm chart to a local cluster and recorded the exact commands and successful readiness/completion evidence.
- [ ] Raw manifest verification and Helm verification were performed independently.
- [ ] Manual verification proves TLS authentication and configured connectivity from the verify pod, or through the verify pod's exposed job/service behavior, to externally hosted CockroachDB and PostgreSQL.
- [ ] If real external CockroachDB and PostgreSQL endpoints were unavailable, the task is left failed with the exact blocker recorded rather than using in-cluster databases or fake coverage.
- [ ] Concise operator-facing usage notes explain how to apply the raw manifests and how to install the Helm chart.
- [ ] Any swallowed/ignored error anti-pattern discovered during this work has a bug task created via `add-bug`.
- [ ] `make check` — passes cleanly, or the task fails with the full failing output recorded.
- [ ] `make lint` — passes cleanly, or the task fails with the full failing output recorded.
</acceptance_criteria>
