# buildfix-receipts

Tolerant receipt loader that reads sensor reports from `artifacts/*/report.json`.

## Build & Test

```bash
cargo test -p buildfix-receipts
cargo clippy -p buildfix-receipts
```

## Key Functions

### `load_receipts(artifacts_dir: &Path) -> Vec<LoadedReceipt>`

Scans the `artifacts/*/report.json` glob pattern and returns all found receipts.

**Behavior:**
- Tolerant: ignores unknown fields in JSON
- Collects parse errors without failing (returns `Result` per receipt)
- Deterministic: sorts results by path
- Handles missing optional fields gracefully

## Types

### `LoadedReceipt`
```rust
struct LoadedReceipt {
    path: PathBuf,           // Full path to report.json
    sensor_id: String,       // Parent directory name (e.g., "builddiag")
    receipt: Result<ReceiptEnvelope, ReceiptLoadError>,
}
```

### `ReceiptLoadError`
- `Io(io::Error)` - File read failure
- `Json(serde_json::Error)` - Parse failure

## Expected Directory Structure

```
artifacts/
  builddiag/
    report.json
  depguard/
    report.json
  buildscan/
    report.json
```

## Invariants

- Never fails on missing directories - returns empty vec
- Sensor ID derived from directory name, not file content
- Path sorting ensures deterministic processing order
