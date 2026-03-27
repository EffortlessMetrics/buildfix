# buildfix-receipts-clippy

[![Crates.io](https://img.shields.io/crates/v/buildfix-receipts-clippy.svg)](https://crates.io/crates/buildfix-receipts-clippy)
[![Documentation](https://docs.rs/buildfix-receipts-clippy/badge.svg)](https://docs.rs/buildfix-receipts-clippy)

**Clippy adapter for buildfix - converts Clippy JSON output to buildfix receipts.**

This crate is an adapter that parses Clippy JSON output and converts it into the standardized buildfix receipt format.

## What is buildfix?

[buildfix](https://github.com/EffortlessMetrics/buildfix) is a receipt-driven repair tool for Cargo workspace hygiene. It consumes sensor receipts and emits deterministic repair plans.

## What is Clippy?

[Clippy](https://github.com/rust-lang/rust-clippy) is a collection of lints to catch common mistakes and improve Rust code. It's the standard linting tool for the Rust ecosystem.

## Usage

### Basic Usage

```rust
use buildfix_receipts_clippy::ClippyAdapter;
use buildfix_adapter_sdk::Adapter;

let adapter = ClippyAdapter::new();
let envelope = adapter.load(path_to_clippy_json)?;
```

### Generating Clippy JSON Output

To generate JSON output that this adapter can consume, run Clippy with the `--message-format=json` flag:

```bash
cargo clippy --message-format=json > clippy-output.jsonl
```

## Check IDs

This adapter generates check IDs in the format: `clippy.<lint-name>`

For example:
- `clippy.unused_imports`
- `clippy.unnecessary_mut_passed`
- `clippy.clippy::style`

## Severity Mapping

Clippy lint levels are mapped to buildfix severity:
- `error` → `Error`
- `warn` → `Warn`
- Other → `Info`

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
