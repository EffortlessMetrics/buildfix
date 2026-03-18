# buildfix-receipts-cargo-deny

Adapter for cargo-deny output. Converts cargo-deny JSON reports to buildfix receipts.

## Build & Test

```bash
cargo test -p buildfix-receipts-cargo-deny
cargo clippy -p buildfix-receipts-cargo-deny
```

## Key Types

### `CargoDenyAdapter`
Implements the `Adapter` trait with `sensor_id()` returning "cargo-deny".

### `CargoDenyReport`
Deserializes cargo-deny JSON output with sections:
- `licenses` - License violations
- `bans` - Banned dependencies
- `advisories` - Security advisories
- `sources` - Untrusted sources

## Check ID Mapping

| cargo-deny ID | buildfix check_id |
|--------------|------------------|
| missing-license | licenses.missing |
| unlicensed | licenses.unlicensed |
| multi-usage | bans.multi-usage |
| circular | bans.circular |
| multiple-versions | bans.multiple-versions |
| wildcard-dependencies | bans.wildcard-dependencies |
| RUSTSEC-* | advisories.RUSTSEC-* |
| untrusted-source | sources.untrusted |

## Severity Mapping

- `deny` entries -> `Severity::Error`
- `warn` entries -> `Severity::Warn`

## Verdict Calculation

- Any errors -> `VerdictStatus::Fail`
- Only warnings -> `VerdictStatus::Warn`
- No findings -> `VerdictStatus::Pass`

## Special Considerations

- Location paths are derived from package names (hyphens replaced with underscores)
- Advisory findings typically have no location (vulnerabilities are package-level)
- Package information stored in finding `data` field as JSON
