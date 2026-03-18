# buildfix-receipts-rustc-json

Adapter for rustc JSON output. Converts rustc's JSON diagnostic output to buildfix receipts.

## Build & Test

```bash
cargo test -p buildfix-receipts-rustc-json
cargo clippy -p buildfix-receipts-rustc-json
```

## Key Types

### `RustcJsonAdapter`
Implements the `Adapter` trait with `sensor_id()` returning "rustc".

## Input Format

Rustc emits JSON diagnostics, one per line:
```json
{"reason": "compiler-message", "package_id": "my_crate 0.1.0", "message": {"code": "unused_imports", "level": "warning", "message": "unused import: `foo`", "spans": [...]}}
```

## Filtered Reasons

Only processes messages with `reason: "compiler-message"`. Skips:
- `compiler-aborted`
- `build-finished`
- (other reasons)

## Severity Mapping

| rustc level | buildfix severity |
|------------|------------------|
| error | Severity::Error |
| warning | Severity::Warn |
| warn | Severity::Warn |
| note | Severity::Info |
| help | Severity::Info |

## Verdict Calculation

- Any errors -> `VerdictStatus::Fail`
- Only warnings -> `VerdictStatus::Warn`
- Only notes/help -> `VerdictStatus::Pass`

## Location Extraction

- Uses first span with non-empty `file_name`
- Extracts `line_start` and `column_start`
- Skips spans with empty filenames

## Special Considerations

- Input is line-delimited JSON (NDJSON), not a single JSON array
- Each line is parsed independently, skipped if invalid
- Uses `message.code` as both `check_id` and `code` fields
- Package ID is available but not currently used in output
