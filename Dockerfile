# syntax=docker/dockerfile:1.7

FROM rust:1.93-bookworm AS chef

WORKDIR /workspace

ARG TARGETARCH

RUN apt-get update \
    && apt-get install --yes --no-install-recommends musl-tools \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef --locked

RUN case "${TARGETARCH}" in \
      amd64) export RUST_TARGET=x86_64-unknown-linux-musl ;; \
      arm64) export RUST_TARGET=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported TARGETARCH=${TARGETARCH}" >&2; exit 1 ;; \
    esac \
    && printf '%s' "${RUST_TARGET}" > /tmp/rust-target \
    && rustup target add "${RUST_TARGET}"

FROM chef AS planner

COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo
COPY crates/ingest-contract/Cargo.toml crates/ingest-contract/Cargo.toml
COPY crates/runner/Cargo.toml crates/runner/Cargo.toml
COPY crates/setup-sql/Cargo.toml crates/setup-sql/Cargo.toml

RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

COPY --from=planner /workspace/recipe.json recipe.json

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/workspace/target \
    export RUST_TARGET="$(cat /tmp/rust-target)" \
    && cargo chef cook --locked --release --target "${RUST_TARGET}" --recipe-path recipe.json

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/workspace/target \
    export RUST_TARGET="$(cat /tmp/rust-target)" \
    && cargo build --locked --release --target "${RUST_TARGET}" -p runner --bin runner \
    && install -D "target/${RUST_TARGET}/release/runner" /runner/runner

FROM scratch AS runtime

COPY --from=builder /runner/runner /usr/local/bin/runner

ENTRYPOINT ["/usr/local/bin/runner"]
