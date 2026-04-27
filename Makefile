.PHONY: check lint test test-long

check:
	nix run .#check

lint:
	nix run .#lint

test:
	nix run .#test

test-long:
	nix run .#test-long
