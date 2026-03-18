# buildfix-receipts-sarif

Adapter for SARIF (Static Analysis Results Interchange Format) output. Converts SARIF log files to buildfix receipts.

## Build & Test

```bash
cargo test -p buildfix-receipts-sarif
cargo clippy -p buildfix-receipts-sarif
```

## Key Types

### `SarifAdapter`
Implements the `Adapter` trait. Supports custom tool names via `with_tool_name()`.

```rust
let adapter = SarifAdapter::new().with_tool_name("clippy");
// sensor_id becomes "sarif-clippy"
```

## SARIF Schema

Parses standard SARIF 2.1.0 format:
- `runs[].tool.driver.name` - Tool name
- `runs[].tool.driver.version` - Tool version
- `runs[].results[].ruleId` - Check ID
- `runs[].results[].level` - Severity (error/warning/note)
- `runs[].results[].message.text` - Finding message
- `runs[].results[].locations[].physicalLocation` - Location info

## Severity Mapping

| SARIF level | buildfix severity |
|------------|------------------|
| error | Severity::Error |
| warning | Severity::Warn |
| note | Severity::Info |
| none | Severity::Info |
| (missing) | Severity::Info |

## Tool Name Derivation

- Default sensor_id: "sarif"
- With `with_tool_name("Semgrep")`: "sarif-semgrep"
- Falls back to "unknown" if driver name is missing

## Location Extraction

Extracts from `physicalLocation.artifactLocation.uri` and `region.startLine`/`startColumn`.

## Special Considerations

- Supports multiple runs - uses last run's tool info
- Sensor ID can be customized for tools that output SARIF format
- URI base IDs are preserved (not resolved)
