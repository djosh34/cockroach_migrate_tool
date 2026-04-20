.PHONY: check lint test test-long

GOTOOLCHAIN ?= auto

check:
	cargo clippy --workspace --all-targets -- -D warnings

lint: check

test:
	cargo test --workspace
	cd cockroachdb_molt/molt && GOTOOLCHAIN=$(GOTOOLCHAIN) go test ./cmd/verifyservice -count=1

test-long:
	cargo test --workspace -- --ignored --test-threads=1
