FROM rust:1.93-bookworm AS builder

WORKDIR /workspace

COPY . .

RUN cargo build --locked --release -p runner --bin runner

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /workspace/target/release/runner /usr/local/bin/runner

ENTRYPOINT ["/usr/local/bin/runner"]
