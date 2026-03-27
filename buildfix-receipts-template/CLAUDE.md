# buildfix-receipts-template

Template adapter for buildfix - demonstrates adapter development patterns.

## Purpose

This crate serves as a **reference implementation** for developers creating new adapters. It demonstrates all best practices and patterns used in the buildfix adapter ecosystem and can be copy-pasted as a starting point for new adapter development.

## Using This Template

### Step 1: Copy the Crate

```bash
cp -r buildfix-receipts-template buildfix-receipts-mytool
```

### Step 2: Rename and Update

Rename the adapter struct and update these files:

| File | Changes |
|------|---------|
| `Cargo.toml` | Update `name`, `description`, `keywords` |
| `src/lib.rs` | Rename `ExampleLinterAdapter` to your adapter name |
| `src/lib.rs` | Update input types to match your tool's JSON schema |
| `src/lib.rs` | Update `sensor_id()` return value |
| `src/lib.rs` | Update `supported_schemas()` return value |
| `tests/fixtures/report.json` | Replace with your tool's actual output |
| `CLAUDE.md` | Update check ID mapping table |
| `README.md` | Update documentation |

### Step 3: Implement Conversion Logic

The key function to modify is `convert_report()`:

1. Update input types (`ExampleLinterReport`, `ExampleLinterFinding`) to match your tool
2. Map your tool's severity levels in `map_severity()`
3. Generate appropriate check IDs in `format_check_id()`
4. Handle any tool-specific fields

## Check ID Mapping

Check IDs follow the format: `<tool>.<category>.<specific>`

### Example Check IDs

| Tool Rule | buildfix Check ID | Description |
|-----------|-------------------|-------------|
| EXAMPLE001 | `example-linter.code.EXAMPLE001` | Example code issue |
| EXAMPLE002 | `example-linter.style.EXAMPLE002` | Example style issue |
| EXAMPLE003 | `example-linter.security.EXAMPLE003` | Example security issue |

### Categories

Common category prefixes:
- `code` - General code issues
- `style` - Code style/formatting
- `security` - Security vulnerabilities
- `performance` - Performance issues
- `correctness` - Logic errors
- `compat` - Compatibility issues

## Severity Mapping

Map your tool's severity levels to buildfix standard levels:

| Tool Severity | buildfix Severity |
|---------------|-------------------|
| error, err, fatal, critical | `Severity::Error` |
| warning, warn, major | `Severity::Warn` |
| info, information, note, minor | `Severity::Info` |
| (unknown) | `Severity::Note` |

## Input Format

This template expects JSON in this format:

```json
{
  "version": "1.0",
  "findings": [
    {
      "rule": "EXAMPLE001",
      "severity": "error",
      "message": "Example finding",
      "file": "src/main.rs",
      "line": 42,
      "column": 10
    }
  ]
}
```

## Testing

Run tests with:

```bash
cargo test -p buildfix-receipts-template
```

### Test Coverage

Ensure your adapter tests cover:
- Basic adapter functionality (`sensor_id`, metadata)
- Empty input (no findings)
- Findings at each severity level
- Path normalization
- Check ID format validation
- Location field handling

## Files to Modify

When creating a new adapter based on this template:

1. **`Cargo.toml`** - Package metadata and dependencies
2. **`src/lib.rs`** - Main adapter implementation
3. **`tests/adapter_test.rs`** - Integration tests
4. **`tests/fixtures/report.json`** - Test input data
5. **`CLAUDE.md`** - This file (adapter-specific documentation)
6. **`README.md`** - User-facing documentation

## Integration with Workspace

After creating your adapter, add it to the workspace `Cargo.toml`:

```toml
[workspace]
members = [
  # ... existing members ...
  "buildfix-receipts-mytool",
]
```
