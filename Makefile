NIX_CACHE_DIR := $(CURDIR)/.cache/nix

.PHONY: check lint test test-runner test-molt test-long

check:
	XDG_CACHE_HOME="$(NIX_CACHE_DIR)/check" nix run .#check

lint:
	XDG_CACHE_HOME="$(NIX_CACHE_DIR)/lint" nix run .#lint

test:
	XDG_CACHE_HOME="$(NIX_CACHE_DIR)/test" nix run .#test

test-runner:
	XDG_CACHE_HOME="$(NIX_CACHE_DIR)/test-runner" nix run .#test-runner

test-molt:
	XDG_CACHE_HOME="$(NIX_CACHE_DIR)/test-molt" nix run .#test-molt

test-long:
	XDG_CACHE_HOME="$(NIX_CACHE_DIR)/test-long" nix run .#test-long
