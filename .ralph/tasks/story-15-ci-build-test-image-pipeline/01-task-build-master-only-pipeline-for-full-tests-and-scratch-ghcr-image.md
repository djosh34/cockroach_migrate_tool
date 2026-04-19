## Task: Build a push-to-master-only pipeline that runs the full test suite and publishes a commit-tagged scratch image to GHCR <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create the build/test pipeline requested by the PO without broadening scope into unrelated release automation. The pipeline must run only on pushes to `master`, run the full test suite, build the production container image as a scratch final image containing only the binary, tag the published OCI image with the commit identity, and push that image to GHCR. The higher order goal is to make production-image creation deterministic, minimal, and tied to code that has actually passed the complete suite, while explicitly avoiding automatic `latest` or version tagging.

In scope:
- CI workflow definition for push events to `master` only
- no workflow execution on pull requests
- full repository validation in CI, including the complete test suite rather than a reduced smoke subset
- build pipeline that is free to use multi-stage or non-scratch builder stages, but must produce a scratch final runtime image
- final image contents limited to the compiled binary only
- immutable OCI image tagging derived from the pushed commit
- push to GHCR
- workflow design that keeps a later Quay publish path easy to add without reworking the whole pipeline

Out of scope:
- manual release tagging flows such as `latest` or semantic version tags
- pull-request CI
- publishing to Quay in this task
- broader deployment automation beyond building, testing, and publishing the image

Decisions already made by the PO and required by this task:
- run on push to `master`
- never run on pull requests
- do not tag with `latest`
- do not tag with version tags automatically
- publish a commit-tagged OCI image
- publish to GHCR now
- keep future Quay support in mind
- the final image must be `scratch`
- no Alpine in the final step
- the runtime image should contain just the binary and nothing else

The implementation must make it hard to accidentally publish an untested or differently-built image than the one produced by the guarded `master` push workflow.

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers workflow behavior, trigger scope, image-tagging rules, and scratch-image contract
- [ ] The workflow triggers only on pushes to `master` and does not run on pull requests
- [ ] The CI path runs the full validation suite required by the repository rather than a reduced subset
- [ ] The publish step pushes a commit-tagged OCI image to GHCR and does not publish `latest` or version tags
- [ ] The final runtime image is a scratch image whose runtime payload is only the application binary
- [ ] The workflow structure leaves a clean extension point for adding Quay publishing later without mixing registry-specific logic through the whole file
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] `make test-long` — passes cleanly
</acceptance_criteria>
