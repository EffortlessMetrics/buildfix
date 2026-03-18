# buildfix-adapter-sdk

SDK for building intake adapters that convert sensor outputs to buildfix receipts.

## Build & Test

```bash
cargo test -p buildfix-adapter-sdk
cargo clippy -p buildfix-adapter-sdk
```

## Key Types

### `Adapter` trait
```rust
pub trait Adapter: Send + Sync {
    fn sensor_id(&self) -> &str;
    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError>;
}
```

### `AdapterError`
- `Io(std::io::Error)` - File read failures
- `Json(serde_json::Error)` - Parse failures
- `InvalidFormat(String)` - Invalid sensor output format
- `MissingField(String)` - Required fields missing

### `AdapterTestHarness<A: Adapter>`
Test utility for validating adapter implementations:
- `validate_receipt()` - Validates receipt structure
- `validate_receipt_fixture()` - Loads and validates a fixture file
- `golden_test()` - Compares against expected output
- `assert_finding_count()` - Verifies finding counts
- `assert_has_check_id()` - Checks for specific check IDs
- `extract_check_ids()` - Extracts all check IDs from findings

### `ReceiptBuilder`
Builder pattern for constructing `ReceiptEnvelope` instances with sensible defaults.

## Creating a New Adapter

Implement the `Adapter` trait for your sensor-specific adapter:

```rust
use buildfix_adapter_sdk::{Adapter, AdapterError};
use buildfix_types::receipt::ReceiptEnvelope;
use std::path::Path;

pub struct MySensorAdapter {
    sensor_id: String,
}

impl Adapter for MySensorAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        // Parse sensor output and convert to ReceiptEnvelope
    }
}
```

## Special Considerations

- Adapters must be `Send + Sync` for concurrent use
- The `sensor_id()` should return a unique identifier (e.g., "cargo-deny", "sarif")
- Use `ReceiptBuilder` to construct receipts with proper schema and defaults
- All findings should have meaningful `check_id` values for actionable fixes
