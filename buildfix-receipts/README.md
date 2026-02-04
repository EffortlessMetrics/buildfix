# buildfix-receipts

Tolerant loader for sensor receipt files from `artifacts/*/report.json`.

## Features

- Scans `artifacts/*/report.json` glob pattern
- Tolerant parsing: ignores unknown fields, collects errors without failing
- Deterministic: results sorted by path
- Graceful handling of missing optional fields

## Usage

```rust
use buildfix_receipts::load_receipts;

let receipts = load_receipts(Path::new("artifacts"));
for loaded in receipts {
    match loaded.receipt {
        Ok(envelope) => println!("Loaded {} findings from {}",
            envelope.findings.len(), loaded.sensor_id),
        Err(e) => eprintln!("Failed to parse {}: {}", loaded.path.display(), e),
    }
}
```

## Types

### `LoadedReceipt`
```rust
struct LoadedReceipt {
    path: PathBuf,           // Full path to report.json
    sensor_id: String,       // Directory name (e.g., "builddiag")
    receipt: Result<ReceiptEnvelope, ReceiptLoadError>,
}
```

### `ReceiptLoadError`
- `Io` - File read failure
- `Json` - Parse failure

## Expected Directory Structure

```
artifacts/
  builddiag/report.json
  depguard/report.json
  buildscan/report.json
```

This crate is part of the [buildfix](https://github.com/EffortlessMetrics/buildfix) workspace.
