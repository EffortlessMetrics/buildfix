# buildfix-receipts-sarif

[![Crates.io](https://img.shields.io/crates/v/buildfix-receipts-sarif.svg)](https://crates.io/crates/buildfix-receipts-sarif)
[![Documentation](https://docs.rs/buildfix-receipts-sarif/badge.svg)](https://docs.rs/buildfix-receipts-sarif)

**SARIF adapter for buildfix - converts Static Analysis Results Interchange Format to buildfix receipts.**

This crate is an adapter that parses SARIF (Static Analysis Results Interchange Format) files and converts them into the standardized buildfix receipt format.

## What is buildfix?

[buildfix](https://github.com/EffortlessMetrics/buildfix) is a receipt-driven repair tool for Cargo workspace hygiene. It consumes sensor receipts and emits deterministic repair plans.

## What is SARIF?

SARIF (Static Analysis Results Interchange Format) is an industry-standard format for static analysis tool results. It's used by tools like GitHub CodeQL, Semgrep, and many other static analyzers.

## Usage

### Basic Usage

```rust
use buildfix_receipts_sarif::SarifAdapter;
use buildfix_adapter_sdk::Adapter;

let adapter = SarifAdapter::new();
let envelope = adapter.load(path_to_sarif_file)?;
```

### With Custom Tool Name

```rust
use buildfix_receipts_sarif::SarifAdapter;
use buildfix_adapter_sdk::Adapter;

let adapter = SarifAdapter::new().with_tool_name("semgrep");
let envelope = adapter.load(path_to_sarif_file)?;
```

## Check IDs

This adapter generates check IDs in the format: `sarif.<rule-id>` or `sarif-<tool>.<rule-id>` when a custom tool name is provided.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
