# Installation

Supported shipped artifacts:

- `runner`
- `verify`

The repository no longer publishes a separate SQL-emitter image or binary.

## Build From Source

```bash
cargo build --release -p runner
```

`verify` is built from `cockroachdb_molt/molt`.

## Container Images

Use the published `runner` and `verify` images from your registry of choice. The root `Dockerfile` builds the `runner` image.

## Validation

Recommended fast checks after local changes:

```bash
make check
make test
```
