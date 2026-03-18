# buildfix-receipts-depguard

Adapter for depguard output. Converts depguard JSON reports to buildfix receipts.

## Build & Test

```bash
cargo test -p buildfix-receipts-depguard
cargo clippy -p buildfix-receipts-depguard
```

## Key Types

### `DepguardAdapter`
Implements the `Adapter` trait with `sensor_id()` returning "depguard".

## Supported Formats

The adapter supports two depguard JSON output formats:

### Array Format
```json
[
  {
    "manifest_path": "/path/to/Cargo.toml",
    "violations": [
      { "dependency": "foo", "type": "path_requires_version" }
    ]
  }
]
```

### Files Format
```json
{
  "files": [
    {
      "path": "/path/to/Cargo.toml",
      "messages": [
        { "message": "...", "code": "E001", "type": "...", "line": 10, "column": 5 }
      ]
    }
  ]
}
```

## Check ID Mapping

| depguard type | buildfix check_id |
|--------------|------------------|
| path_requires_version | deps.path_requires_version |
| workspace_inheritance | deps.workspace_inheritance |
| duplicate_dependency_versions | deps.duplicate_dependency_versions |
| duplicate_versions | deps.duplicate_dependency_versions |
| (other) | deps.{type} |

## Severity Mapping

- All violations -> `Severity::Warn`
- No errors (depguard only reports warnings)

## Verdict Calculation

- Any violations -> `VerdictStatus::Warn`
- No findings -> `VerdictStatus::Pass`

## Special Considerations

- Automatically detects input format (array vs files)
- Array format: location uses `manifest_path`, no line/column
- Files format: location includes path, line, and column from messages
- Violation details stored in finding `data` field
- Message format: "depguard: {violation_type} - {dependency}"
