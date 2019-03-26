# CorTeX Peripherals - Rust+ZMQ implementation

Worker implementations and utilities for CorTeX, in Rust.

### Workers

1. [Engrafo](https://github.com/arxiv-vanity/engrafo) - tex-to-html conversion via latexml, with advanced styling and UX
  - uses a dedicated `docker` image which is an installation prerequisite.
  - builds under the `engrafo` feature flag, via `cargo test --features=engrafo`
  - starting a worker: `cargo run --release --features=engrafo --bin engrafo_worker`
