pub struct RustWorkspaceImageCacheExpectation<'a> {
    pub dockerfile_label: &'a str,
    pub build_command: &'a str,
}

pub struct RustWorkspaceImageCacheContract;

impl RustWorkspaceImageCacheContract {
    pub fn assert_dependency_first_layers(
        dockerfile_text: &str,
        expectation: RustWorkspaceImageCacheExpectation<'_>,
    ) {
        for required_marker in [
            "# syntax=docker/dockerfile:1.7",
            "FROM rust:1.93-bookworm AS chef",
            "FROM chef AS planner",
            "FROM chef AS builder",
            "COPY Cargo.toml Cargo.lock ./",
            "COPY .cargo .cargo",
            "COPY crates/ingest-contract/Cargo.toml crates/ingest-contract/Cargo.toml",
            "COPY crates/runner/Cargo.toml crates/runner/Cargo.toml",
            "RUN cargo install cargo-chef --locked",
            "RUN cargo chef prepare --recipe-path recipe.json",
            "COPY --from=planner /workspace/recipe.json recipe.json",
            "cargo chef cook --locked --release --target \"${RUST_TARGET}\" --recipe-path recipe.json",
            "--mount=type=cache,target=/usr/local/cargo/registry",
            "--mount=type=cache,target=/usr/local/cargo/git/db",
            "--mount=type=cache,target=/workspace/target",
            "COPY . .",
            "ARG TARGETARCH",
            "rustup target add \"${RUST_TARGET}\"",
            "x86_64-unknown-linux-musl",
            "aarch64-unknown-linux-musl",
            "unsupported TARGETARCH",
        ] {
            assert!(
                dockerfile_text.contains(required_marker),
                "{} must contain `{required_marker}` to keep Rust dependency planning and build caching explicit",
                expectation.dockerfile_label,
            );
        }

        assert!(
            dockerfile_text.contains(expectation.build_command),
            "{} must compile the expected binary with `{}`",
            expectation.dockerfile_label,
            expectation.build_command,
        );

        assert_strict_order(
            dockerfile_text,
            &[
                "COPY Cargo.toml Cargo.lock ./",
                "COPY crates/ingest-contract/Cargo.toml crates/ingest-contract/Cargo.toml",
                "COPY crates/runner/Cargo.toml crates/runner/Cargo.toml",
                "RUN cargo chef prepare --recipe-path recipe.json",
                "COPY --from=planner /workspace/recipe.json recipe.json",
                "cargo chef cook --locked --release --target \"${RUST_TARGET}\" --recipe-path recipe.json",
                "COPY . .",
                expectation.build_command,
            ],
            expectation.dockerfile_label,
        );
    }
}

fn assert_strict_order(text: &str, ordered_markers: &[&str], dockerfile_label: &str) {
    let mut previous_position = None;
    for marker in ordered_markers {
        let position = text
            .find(marker)
            .unwrap_or_else(|| panic!("{dockerfile_label} must contain `{marker}`"));
        if let Some(previous_position) = previous_position {
            assert!(
                previous_position < position,
                "{dockerfile_label} must keep `{marker}` after the previous dependency-cache boundary marker",
            );
        }
        previous_position = Some(position);
    }
}
