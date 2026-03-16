# buildfix-receipts

Receipt ingestion helpers for buildfix.

This crate scans `artifacts/*/report.json` and loads sensor envelopes with tolerant parsing so planning can proceed even when some inputs fail.

## API

- `load_receipts(artifacts_dir) -> Vec<LoadedReceipt>`

`LoadedReceipt` includes:

- `path`
- `sensor_id`
- `receipt: Result<ReceiptEnvelope, ReceiptLoadError>`

## Behavior

- Reads `artifacts/*/report.json`
- Skips reserved non-sensor directories (`buildfix`, `cockpit`)
- Preserves per-receipt load errors instead of failing the entire batch
- Sorts outputs by path for deterministic downstream processing

## Error types

- `ReceiptLoadError::Io`
- `ReceiptLoadError::Json`

This is a support crate for the `buildfix` workspace and may evolve in lockstep with the workspace release train.
