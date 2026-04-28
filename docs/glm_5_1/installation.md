# Installation

Build and run the supported binaries only:

- `runner`
- `verify`

## Local Build

```bash
cargo build --release -p runner
```

`verify-binary` is the Go `molt` binary built from `cockroachdb_molt/molt` for verify-service use.

## Images

Pull the published `runner` and `verify` images and mount your configs and TLS material into `/config`.
