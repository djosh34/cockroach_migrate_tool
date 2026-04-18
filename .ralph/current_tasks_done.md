# Done Tasks Summary

Generated: Wed Apr  1 03:22:32 PM CEST 2026

# Task `.ralph/tasks/smells/e2e-harness-shared-dcs-boundary.md`

```
## Smell Set: e2e-harness-shared-dcs-boundary <status>done</status> <passes>true</passes>

Please refer to skill `improve-code-boundaries` to see what smells there are.

Inside dirs:
```

==============

# Task `.ralph/tasks/story-bootstrap-runtime-shared-state/context.md`

```
# Story: Bootstrap, Runtime, and Shared State Boundaries

This story captures the design decisions already made in chat so they are not lost:

- There is no second local PostgreSQL config source of truth.
```

==============

# Task `.ralph/tasks/story-bootstrap-runtime-shared-state/design-secrets-and-bootstrap-options.md`

```
## Task: Reconcile the secrets/bootstrap design note with the implemented boundary <status>done</status> <passes>true</passes>

<description>
Must use `tdd` and `improve-code-boundaries` to keep this note aligned with the real public code.
```

==============

# Task `.ralph/tasks/story-bootstrap-runtime-shared-state/task-design-validated-secrets-struct.md`

```
## Task: Design the validated secrets struct and its real consumers <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-freeze-local-config-and-dcs-bootstrap.md</blocked_by>

<description>
Must use tdd skill to complete
```

==============

# Task `.ralph/tasks/story-bootstrap-runtime-shared-state/task-freeze-local-config-and-dcs-bootstrap.md`

```
## Task: Freeze local runtime config and DCS bootstrap ownership <status>done</status> <passes>true</passes>
<priority>ultra_high</priority>

<description>
Must use tdd skill to complete
```

==============

# Task `.ralph/tasks/story-bootstrap-runtime-shared-state/task-reduce-shared-types-and-pub-surface.md`

```
## Task: Reduce shared types and pub surface to the actual public model <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-freeze-local-config-and-dcs-bootstrap.md</blocked_by>

<description>
Must use tdd skill to complete
```

==============

# Task `.ralph/tasks/story-bootstrap-runtime-shared-state/task-reshape-shared-instance-publication-model.md`

```
## Task: Reshape shared instance publication around `InstanceName` and remove separate current-primary state <status>completed</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-freeze-local-config-and-dcs-bootstrap.md</blocked_by>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-reduce-shared-types-and-pub-surface.md</blocked_by>

<description>
```

==============

# Task `.ralph/tasks/story-bootstrap-runtime-shared-state/task-split-types-by-domain-and-remove-duplicate-shapes.md`

```
## Task: Split shared types by domain and remove duplicate shapes <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-reduce-shared-types-and-pub-surface.md</blocked_by>

<description>
Must use tdd skill to complete
```

==============

# Task `.ralph/tasks/story-component-public-boundaries/context.md`

```
# Story: Component Public Boundaries

This story exists to freeze the public method boundaries of the main building blocks before the full harness is implemented.

Shared assumptions already agreed:
```

==============

# Task `.ralph/tasks/story-component-public-boundaries/task-reshape-dcs-role-scoped-contexts.md`

```
## Task: Reshape DCS into role-scoped contexts with self-derived identity <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-freeze-local-config-and-dcs-bootstrap.md</blocked_by>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-reduce-shared-types-and-pub-surface.md</blocked_by>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-reshape-shared-instance-publication-model.md</blocked_by>
```

==============

# Task `.ralph/tasks/story-component-public-boundaries/task-reshape-logger-logctx-boundary.md`

```
## Task: Reshape logger around a private LogCtx boundary <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-reduce-shared-types-and-pub-surface.md</blocked_by>

<description>
Must use tdd skill to complete
```

==============

# Task `.ralph/tasks/story-component-public-boundaries/task-reshape-pg-config-render-write-boundary.md`

```
## Task: Reshape pg_config into a private-render apply boundary <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-reduce-shared-types-and-pub-surface.md</blocked_by>

<description>
Must use tdd skill to complete
```

==============

# Task `.ralph/tasks/story-component-public-boundaries/task-reshape-pg-ctl-lifecycle-boundary.md`

```
## Task: Reshape pg_ctl into a pure lifecycle controller boundary <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-freeze-local-config-and-dcs-bootstrap.md</blocked_by>

<description>
Must use tdd skill to complete
```

==============

# Task `.ralph/tasks/story-component-public-boundaries/task-reshape-pg-info-reconnecting-context.md`

```
## Task: Reshape pg_info into a reconnecting context with polling methods <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-freeze-local-config-and-dcs-bootstrap.md</blocked_by>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-reduce-shared-types-and-pub-surface.md</blocked_by>

<description>
```

==============

# Task `.ralph/tasks/story-component-public-boundaries/task-reshape-pg-logger-worker-boundary.md`

```
## Task: Reshape pg_logger into a hands-off worker boundary <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-freeze-local-config-and-dcs-bootstrap.md</blocked_by>
<blocked_by>.ralph/tasks/story-component-public-boundaries/task-reshape-logger-logctx-boundary.md</blocked_by>

<description>
```

==============

# Task `.ralph/tasks/story-control-loop-boundaries/context.md`

```
# Story: Control Loop Boundaries

This story defines the public boundary of the two long-running control loops:

- the local instance manager process
```

==============

# Task `.ralph/tasks/story-control-loop-boundaries/task-reshape-instance-manager-control-loop.md`

```
## Task: Reshape instance manager into a private local reconcile loop <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-component-public-boundaries/task-reshape-dcs-role-scoped-contexts.md</blocked_by>
<blocked_by>.ralph/tasks/story-component-public-boundaries/task-reshape-pg-config-render-write-boundary.md</blocked_by>
<blocked_by>.ralph/tasks/story-component-public-boundaries/task-reshape-pg-info-reconnecting-context.md</blocked_by>
<blocked_by>.ralph/tasks/story-component-public-boundaries/task-reshape-pg-ctl-lifecycle-boundary.md</blocked_by>
```

==============

# Task `.ralph/tasks/story-control-loop-boundaries/task-reshape-operator-leader-loop.md`

```
## Task: Reshape operator into the leader-only shared-state authority <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-freeze-local-config-and-dcs-bootstrap.md</blocked_by>
<blocked_by>.ralph/tasks/story-component-public-boundaries/task-reshape-dcs-role-scoped-contexts.md</blocked_by>
<blocked_by>.ralph/tasks/story-control-loop-boundaries/task-reshape-instance-manager-control-loop.md</blocked_by>
```

==============

# Task `.ralph/tasks/story-e2e-ha-scenarios/context.md`

```
# Story: E2E HA Scenarios

This story defines the full-system docker-backed e2e layer.

Shared assumptions already agreed:
```

==============

# Task `.ralph/tasks/story-e2e-ha-scenarios/task-add-e2e-kill-all-startup-single-primary.md`

```
## Task: Add the e2e scenario for killing all nodes and restarting to exactly one primary <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-e2e-ha-scenarios/task-build-e2e-cluster-harness-on-shared-docker-helper.md</blocked_by>

<description>
Must use tdd skill to complete
```

==============

# Task `.ralph/tasks/story-e2e-ha-scenarios/task-add-e2e-kill-one-container-failover.md`

```
## Task: Add the e2e scenario for killing one container and failing over to quorum <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-e2e-ha-scenarios/task-build-e2e-cluster-harness-on-shared-docker-helper.md</blocked_by>

<description>
Must use tdd skill to complete
```

==============

# Task `.ralph/tasks/story-e2e-ha-scenarios/task-add-e2e-kill-two-return-one.md`

```
## Task: Add the e2e scenario for killing two nodes and returning one <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-e2e-ha-scenarios/task-build-e2e-cluster-harness-on-shared-docker-helper.md</blocked_by>

<description>
Must use tdd skill to complete
```

==============

# Task `.ralph/tasks/story-e2e-ha-scenarios/task-add-e2e-network-split-majority-primary.md`

```
## Task: Add the e2e scenario for network split with minority non-primary and majority one primary <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-e2e-ha-scenarios/task-build-e2e-cluster-harness-on-shared-docker-helper.md</blocked_by>

<description>
Must use tdd skill to complete
```

==============

# Task `.ralph/tasks/story-e2e-ha-scenarios/task-build-e2e-cluster-harness-on-shared-docker-helper.md`

```
## Task: Build the e2e cluster harness on top of the shared docker helper <status>done</status> <passes>true</passes>
<priority>ultra_high</priority>
<blocked_by>.ralph/tasks/story-real-dependency-integration-tests/task-create-shared-docker-test-harness.md</blocked_by>
<blocked_by>.ralph/tasks/story-runtime-composition-and-http-surface/task-design-top-level-runtime-composition.md</blocked_by>
<blocked_by>.ralph/tasks/story-http-crate-and-transport-boundaries/task-wire-operator-and-instance-route-ownership.md</blocked_by>
```

==============

# Task `.ralph/tasks/story-final-boundary-cleanup/context.md`

```
# Story: Final Boundary Cleanup

This story exists for the final hard cleanup pass after the main boundary, runtime, and test stories are complete.

Shared assumptions already agreed:
```

==============

# Task `.ralph/tasks/story-final-boundary-cleanup/task-run-final-boundary-cleanup-pass.md`

```
## Task: Run the final boundary cleanup pass with improve-code-boundaries <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-http-crate-and-transport-boundaries/task-wire-operator-and-instance-route-ownership.md</blocked_by>
<blocked_by>.ralph/tasks/story-real-dependency-integration-tests/task-add-dcs-real-etcd-integration-tests.md</blocked_by>
<blocked_by>.ralph/tasks/story-real-dependency-integration-tests/task-add-pg-info-real-postgres16-integration-tests.md</blocked_by>
<blocked_by>.ralph/tasks/story-real-dependency-integration-tests/task-add-pg-logger-real-postgres16-integration-tests.md</blocked_by>
```

==============

# Task `.ralph/tasks/story-http-crate-and-transport-boundaries/context.md`

```
# Story: HTTP Crate and Transport Boundaries

This story exists to stop HTTP transport concerns from leaking into shared domain crates while still enabling one shared HTTP server.

Shared assumptions already agreed:
```

==============

# Task `.ralph/tasks/story-http-crate-and-transport-boundaries/task-build-shared-axum-rustls-server-and-auth-middleware.md`

```
## Task: Build the shared axum and rustls server with one auth middleware <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-http-crate-and-transport-boundaries/task-create-http-crate-and-move-transport-dtos.md</blocked_by>
<blocked_by>.ralph/tasks/story-runtime-composition-and-http-surface/task-design-shared-http-surface.md</blocked_by>

<description>
```

==============

# Task `.ralph/tasks/story-http-crate-and-transport-boundaries/task-create-http-crate-and-move-transport-dtos.md`

```
## Task: Create the HTTP crate and move transport DTOs out of shared types <status>done</status> <passes>true</passes>
<priority>ultra_high</priority>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-reduce-shared-types-and-pub-surface.md</blocked_by>
<blocked_by>.ralph/tasks/story-bootstrap-runtime-shared-state/task-split-types-by-domain-and-remove-duplicate-shapes.md</blocked_by>
```

==============

# Task `.ralph/tasks/story-http-crate-and-transport-boundaries/task-wire-operator-and-instance-route-ownership.md`

```
## Task: Wire explicit operator and instance-manager route ownership into the shared HTTP crate <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-http-crate-and-transport-boundaries/task-build-shared-axum-rustls-server-and-auth-middleware.md</blocked_by>
<blocked_by>.ralph/tasks/story-control-loop-boundaries/task-reshape-instance-manager-control-loop.md</blocked_by>
<blocked_by>.ralph/tasks/story-control-loop-boundaries/task-reshape-operator-leader-loop.md</blocked_by>
```

==============

# Task `.ralph/tasks/story-jepsen-system-validation/context.md`

```
# Story: Jepsen System Validation

This story captures the later-stage consistency and safety campaign against the whole system.

Shared assumptions already agreed:
```

==============

# Task `.ralph/tasks/story-jepsen-system-validation/task-create-jepsen-runner-and-workload.md`

```
## Task: Create the Jepsen runner and workload for whole-system validation <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-e2e-ha-scenarios/task-add-e2e-kill-one-container-failover.md</blocked_by>
<blocked_by>.ralph/tasks/story-e2e-ha-scenarios/task-add-e2e-kill-two-return-one.md</blocked_by>
<blocked_by>.ralph/tasks/story-e2e-ha-scenarios/task-add-e2e-kill-all-startup-single-primary.md</blocked_by>
<blocked_by>.ralph/tasks/story-e2e-ha-scenarios/task-add-e2e-network-split-majority-primary.md</blocked_by>
```

==============

# Task `.ralph/tasks/story-jepsen-system-validation/task-run-jepsen-and-capture-followup-bugs.md`

```
## Task: Run Jepsen against the system and capture follow-up bugs <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-jepsen-system-validation/task-create-jepsen-runner-and-workload.md</blocked_by>

<description>
Must use tdd skill to complete
```

==============

# Task `.ralph/tasks/story-real-dependency-integration-tests/context.md`

```
# Story: Real Dependency Integration Tests

This story defines the docker-backed integration test layer for individual crates.

Shared assumptions already agreed:
```

==============

# Task `.ralph/tasks/story-real-dependency-integration-tests/task-add-dcs-real-etcd-integration-tests.md`

```
## Task: Add DCS integration tests against real etcd <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-real-dependency-integration-tests/task-create-shared-docker-test-harness.md</blocked_by>
<blocked_by>.ralph/tasks/story-component-public-boundaries/task-reshape-dcs-role-scoped-contexts.md</blocked_by>

<description>
```

==============

# Task `.ralph/tasks/story-real-dependency-integration-tests/task-add-pg-info-real-postgres16-integration-tests.md`

```
## Task: Add pg_info integration tests against real PostgreSQL 16 <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-real-dependency-integration-tests/task-create-shared-docker-test-harness.md</blocked_by>
<blocked_by>.ralph/tasks/story-component-public-boundaries/task-reshape-pg-info-reconnecting-context.md</blocked_by>

<description>
```

==============

# Task `.ralph/tasks/story-real-dependency-integration-tests/task-add-pg-logger-real-postgres16-integration-tests.md`

```
## Task: Add pg_logger integration tests against real PostgreSQL 16 logs <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-real-dependency-integration-tests/task-create-shared-docker-test-harness.md</blocked_by>
<blocked_by>.ralph/tasks/story-component-public-boundaries/task-reshape-pg-logger-worker-boundary.md</blocked_by>

<description>
```

==============

# Task `.ralph/tasks/story-real-dependency-integration-tests/task-create-shared-docker-test-harness.md`

```
## Task: Create the shared docker test harness using testcontainers and normal client libraries <status>done</status> <passes>true</passes>
<priority>high</priority>

<description>
Must use tdd skill to complete
```

==============

# Task `.ralph/tasks/story-runtime-composition-and-http-surface/context.md`

```
# Story: Runtime Composition and HTTP Surface

This story captures the top-level runtime boundary that sits above the operator and instance manager crates.

Current agreed pressure points:
```

==============

# Task `.ralph/tasks/story-runtime-composition-and-http-surface/task-design-shared-http-surface.md`

```
## Task: Design the shared HTTP surface for operator and instance manager behavior <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-control-loop-boundaries/task-reshape-instance-manager-control-loop.md</blocked_by>
<blocked_by>.ralph/tasks/story-control-loop-boundaries/task-reshape-operator-leader-loop.md</blocked_by>

<description>
```

==============

# Task `.ralph/tasks/story-runtime-composition-and-http-surface/task-design-top-level-runtime-composition.md`

```
## Task: Design the top-level runtime composition for two independent loops <status>done</status> <passes>true</passes>
<blocked_by>.ralph/tasks/story-control-loop-boundaries/task-reshape-instance-manager-control-loop.md</blocked_by>
<blocked_by>.ralph/tasks/story-control-loop-boundaries/task-reshape-operator-leader-loop.md</blocked_by>

<description>
```

