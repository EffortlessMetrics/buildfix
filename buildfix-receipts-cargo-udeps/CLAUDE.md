# buildfix-receipts-cargo-udeps

Adapter for cargo-udeps output. Converts cargo-udeps JSON reports to buildfix receipts.

## Build & Test

```bash
cargo test -p buildfix-receipts-cargo-udeps
cargo clippy -p buildfix-receipts-cargo-udeps
```

## Key Types

### `CargoUdepsAdapter`
Implements the `Adapter` trait with `sensor_id()` returning "cargo-udeps".

### `CargoUdepsReport`
```rust
struct CargoUdepsReport {
    success: bool,
    packages: Option<Vec<UdepsPackage>>,
}

struct UdepsPackage {
    manifest_path: String,
    name: String,
    version: String,
    edition: Option<String>,
    kind: Vec<String>,  // "Normal", "Dev", "Build"
}
```

## Check ID Mapping

| Package kind | buildfix check_id |
|-------------|------------------|
| Normal | deps.unused_dependency |
| Dev | deps.unused_dependency |
| Build | deps.unused_build_dependency |

## Severity Mapping

- All unused dependencies -> `Severity::Warn`
- No errors (cargo-udeps only reports warnings)

## Verdict Calculation

- Any unused dependencies -> `VerdictStatus::Warn`
- No findings -> `VerdictStatus::Pass`

## Special Considerations

- Location uses `manifest_path` (Cargo.toml path)
- Package details (name, version, edition, kind) stored in finding `data` field
- Message format: "unused {name}:{version}"
