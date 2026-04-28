# Installation

Supported shipped artifacts:

- `runner`
- `verify`

The repository no longer publishes a separate SQL-emitter image or binary.

## Build From Source

```bash
nix build .#runner
nix build .#verify
```

`runner` is built through crane. `verify` is the Go `molt` binary used by both the local `verify-service` app and the verify image.

## Container Images

Use the published `runner` and `verify` images from your registry of choice. Local source builds are Nix-native; image-generation workflow is handled separately from this installation guide.

## Validation

Recommended fast checks after local changes:

```bash
nix run .#check
nix run .#test
```
