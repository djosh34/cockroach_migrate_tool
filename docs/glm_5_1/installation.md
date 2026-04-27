# Installation

Build and run the supported binaries only:

- `runner`
- `verify`

## Local Build

```bash
cargo build --release -p runner
```

`verify` is built from the Go service under `cockroachdb_molt/molt`.

## Images

Pull the published `runner` and `verify` images and mount your configs and TLS material into `/config`.
