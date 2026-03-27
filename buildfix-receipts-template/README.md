# buildfix-receipts-template

[![Crates.io](https://img.shields.io/crates/v/buildfix-receipts-template.svg)](https://crates.io/crates/buildfix-receipts-template)
[![Documentation](https://docs.rs/buildfix-receipts-template/badge.svg)](https://docs.rs/buildfix-receipts-template)

**Template adapter for buildfix - demonstrates adapter development patterns.**

This crate is a **reference implementation** for developers creating new adapters for the buildfix ecosystem. It demonstrates all best practices and can be used as a starting point for new adapter development.

## What is buildfix?

[buildfix](https://github.com/EffortlessMetrics/buildfix) is a receipt-driven repair tool for Cargo workspace hygiene. It consumes sensor receipts and emits deterministic repair plans.

## What is an Adapter?

An adapter transforms a sensor tool's native output format into the standardized buildfix receipt format (`ReceiptEnvelope`). This allows buildfix to work with any linting, analysis, or monitoring tool.

## Using This Template

### Quick Start

1. **Copy the template:**
   ```bash
   cp -r buildfix-receipts-template buildfix-receipts-mytool
   cd buildfix-receipts-mytool
   ```

2. **Update `Cargo.toml`:**
   ```toml
   [package]
   name = "buildfix-receipts-mytool"
   description = "Adapter for mytool - converts output to buildfix receipts"
   keywords = ["buildfix", "adapter", "mytool"]
   ```

3. **Rename the adapter struct in `src/lib.rs`:**
   ```rust
   pub struct MyToolAdapter {
       sensor_id: &'static str,
   }
   ```

4. **Update input types to match your tool's JSON schema:**
   ```rust
   #[derive(Debug, Deserialize)]
   struct MyToolReport {
       // Fields matching your tool's output
   }
   ```

5. **Implement the conversion logic in `convert_report()`:**
   - Map severity levels
   - Generate check IDs
   - Create findings with locations

6. **Add test fixtures in `tests/fixtures/report.json`:**
   - Use actual output from your tool

7. **Run tests:**
   ```bash
   cargo test
   ```

### Example: Creating a New Adapter

```rust
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct MyToolAdapter;

impl Adapter for MyToolAdapter {
    fn sensor_id(&self) -> &str {
        "my-tool"
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        let report: MyToolReport = serde_json::from_str(&content).map_err(AdapterError::Json)?;
        convert_report(report)
    }
}

impl AdapterMetadata for MyToolAdapter {
    fn name(&self) -> &str { "my-tool" }
    fn version(&self) -> &str { env!("CARGO_PKG_VERSION") }
    fn supported_schemas(&self) -> &[&str] { &["my-tool.report.v1"] }
}
```

## Key Concepts

### Check IDs

Check IDs follow the format: `<tool>.<category>.<specific>`

Examples:
- `clippy.style.unused_variable`
- `cargo-deny.security.CVE-2023-1234`
- `machete.unused_dependency`

### Severity Mapping

| Tool Severity | buildfix Severity |
|---------------|-------------------|
| error, fatal  | `Error` |
| warning, warn | `Warn` |
| info, note    | `Info` |

### Path Normalization

Paths should be:
- Relative to repository root
- Using forward slashes (`/`)
- Without leading `./`

## Testing

The template includes comprehensive tests:

```bash
# Run unit tests
cargo test

# Run with verbose output
cargo test -- --nocapture

# Run specific test
cargo test test_adapter_loads_receipt_from_fixture
```

## Documentation

- [`CLAUDE.md`](CLAUDE.md) - Adapter development guide
- [API Documentation](https://docs.rs/buildfix-receipts-template)

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
