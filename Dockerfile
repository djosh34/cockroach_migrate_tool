FROM rust:1.93-bookworm AS builder

WORKDIR /workspace

ARG TARGETARCH

RUN apt-get update \
    && apt-get install --yes --no-install-recommends musl-tools \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN case "${TARGETARCH}" in \
      amd64) export RUNNER_TARGET=x86_64-unknown-linux-musl ;; \
      arm64) export RUNNER_TARGET=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported TARGETARCH=${TARGETARCH}" >&2; exit 1 ;; \
    esac \
    && rustup target add "${RUNNER_TARGET}" \
    && cargo build --locked --release --target "${RUNNER_TARGET}" -p runner --bin runner \
    && install -D "target/${RUNNER_TARGET}/release/runner" /runner/runner

FROM scratch AS runtime

COPY --from=builder /runner/runner /usr/local/bin/runner

ENTRYPOINT ["/usr/local/bin/runner"]
