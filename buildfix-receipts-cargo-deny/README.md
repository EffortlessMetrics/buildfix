# buildfix-receipts-cargo-deny

[![Crates.io](https://img.shields.io/crates/v/buildfix-receipts-cargo-deny.svg)](https://crates.io/crates/buildfix-receipts-cargo-deny)
[![Documentation](https://docs.rs/buildfix-receipts-cargo-deny/badge.svg)](https://docs.rs/buildfix-receipts-cargo-deny)

**cargo-deny adapter for buildfix - converts cargo-deny JSON output to buildfix receipts.**

This crate is an adapter that parses cargo-deny JSON output and converts it into the standardized buildfix receipt format.

## What is buildfix?

[buildfix](https://github.com/EffortlessMetrics/buildfix) is a receipt-driven repair tool for Cargo workspace hygiene. It consumes sensor receipts and emits deterministic repair plans.

## What is cargo-deny?

[cargo-deny](https://github.com/EmbarkStudios/cargo-deny) is a Cargo plugin that helps you manage your dependencies. It can:
- Check for security vulnerabilities
- Detect duplicate dependencies
- Validate license compliance
- Ban specific crates or sources
- Check for unmaintained or yanked crates

## Usage

### Basic Usage

```rust
use buildfix_receipts_cargo_deny::CargoDenyAdapter;
use buildfix_adapter_sdk::Adapter;

let adapter = CargoDenyAdapter::new();
let envelope = adapter.load(path_to_cargo_deny_json)?;
```

### Generating cargo-deny JSON Output

To generate JSON output that this adapter can consume, run cargo-deny with the `--format json` flag:

```bash
cargo deny check --format json > cargo-deny-output.json
```

## Check IDs

This adapter generates check IDs based on the cargo-deny check type:
- `cargo-deny.licenses` - License violations
- `cargo-deny.bans` - Banned crate usage
- `cargo-deny.advisories` - Security advisories
- `cargo-deny.sources` - Source restrictions

## Severity Mapping

cargo-deny error levels are mapped to buildfix severity:
- `error` → `Error`
- `warn` → `Warn`
- Other → `Info`

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
