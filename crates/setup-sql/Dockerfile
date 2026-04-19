FROM rust:1.93-bookworm AS builder

WORKDIR /workspace

ARG TARGETARCH

RUN apt-get update \
    && apt-get install --yes --no-install-recommends musl-tools \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN case "${TARGETARCH}" in \
      amd64) export SETUP_SQL_TARGET=x86_64-unknown-linux-musl ;; \
      arm64) export SETUP_SQL_TARGET=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported TARGETARCH=${TARGETARCH}" >&2; exit 1 ;; \
    esac \
    && rustup target add "${SETUP_SQL_TARGET}" \
    && cargo build --locked --release --target "${SETUP_SQL_TARGET}" -p setup-sql --bin setup-sql \
    && install -D "target/${SETUP_SQL_TARGET}/release/setup-sql" /setup-sql/setup-sql

FROM scratch AS runtime

COPY --from=builder /setup-sql/setup-sql /usr/local/bin/setup-sql

ENTRYPOINT ["/usr/local/bin/setup-sql"]
