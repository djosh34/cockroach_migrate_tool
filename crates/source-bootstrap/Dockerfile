FROM rust:1.93-bookworm AS builder

WORKDIR /workspace

ARG TARGETARCH

RUN apt-get update \
    && apt-get install --yes --no-install-recommends musl-tools \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN case "${TARGETARCH}" in \
      amd64) export SOURCE_BOOTSTRAP_TARGET=x86_64-unknown-linux-musl ;; \
      arm64) export SOURCE_BOOTSTRAP_TARGET=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported TARGETARCH=${TARGETARCH}" >&2; exit 1 ;; \
    esac \
    && rustup target add "${SOURCE_BOOTSTRAP_TARGET}" \
    && cargo build --locked --release --target "${SOURCE_BOOTSTRAP_TARGET}" -p source-bootstrap --bin source-bootstrap \
    && install -D "target/${SOURCE_BOOTSTRAP_TARGET}/release/source-bootstrap" /source-bootstrap/source-bootstrap

FROM scratch AS runtime

COPY --from=builder /source-bootstrap/source-bootstrap /usr/local/bin/source-bootstrap

ENTRYPOINT ["/usr/local/bin/source-bootstrap"]
