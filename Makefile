.PHONY: check lint test test-long

check:
	cargo clippy --workspace --all-targets -- -D warnings

lint: check

test:
	cargo test --workspace

test-long:
	cargo test --workspace -- --ignored
