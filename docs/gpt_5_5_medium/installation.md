# Installation

Use the published `runner` and `verify` images or build `runner` locally with Cargo.

```bash
cargo build --release -p runner
```

Mount your configs and TLS files into `/config` when starting either service.
