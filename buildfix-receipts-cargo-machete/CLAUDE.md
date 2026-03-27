# buildfix-receipts-cargo-machete

Adapter for cargo-machete output. Converts cargo-machete JSON reports to buildfix receipts.

## Build & Test

```bash
cargo test -p buildfix-receipts-cargo-machete
cargo clippy -p buildfix-receipts-cargo-machete
```

## Key Types

### `CargoMacheteAdapter`
Implements the `Adapter` trait with `sensor_id()` returning "cargo-machete".

### `MacheteReport`
```rust
struct MacheteReport {
    crates: Option<Vec<MacheteCrate>>,
}

struct MacheteCrate {
    name: String,
    manifest_path: String,
    kind: String,  // "direct", "transitive"
}
```

## Check ID Mapping

- All unused dependencies use: `machete.unused_dependency`

## Severity Mapping

- All findings -> `Severity::Warn`
- No errors (cargo-machete only reports warnings)

## Verdict Calculation

- Any unused crates -> `VerdictStatus::Warn`
- No findings -> `VerdictStatus::Pass`

## Special Considerations

- Location uses `manifest_path` (Cargo.toml path)
- Crate details (name, kind) stored in finding `data` field
- Message format: "unused dependency: {name} (kind: {kind})"
- Both "direct" and "transitive" kinds use the same check_id
