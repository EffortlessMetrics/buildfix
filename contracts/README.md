# Cockpit Ecosystem Contracts

This directory contains the shared contracts for the Cockpit sensor ecosystem.

## Directory Structure

```
contracts/
├── schemas/           # JSON Schema definitions
│   ├── sensor.report.v1.json    # Universal sensor output envelope
│   └── cockpit.report.v1.json   # Director aggregate output
├── docs/              # Contract documentation
│   ├── artifact-layout.md       # Filesystem path conventions
│   ├── identity.md              # check_id and code naming rules
│   └── vocabulary.md            # Frozen enum definitions
└── fixtures/          # Test fixtures for schema validation
    ├── pass/          # Valid reports with pass status
    ├── fail/          # Valid reports with fail status
    ├── skip/          # Valid reports with skip status
    └── tool-error/    # Valid reports from crashed tools
```

## Schema Versioning

Schemas follow semantic versioning embedded in the schema name:
- `sensor.report.v1` - First stable version
- `cockpit.report.v1` - First stable version

Schema identifiers must match the pattern: `^[a-z][a-z0-9_-]*\.report\.v[0-9]+$`

## Vocabulary

The ecosystem uses frozen vocabularies for interoperability:

### Verdict Status
- `pass` - All checks passed
- `warn` - Non-blocking issues found
- `fail` - Blocking issues found
- `skip` - Sensor did not run (preconditions unmet)

### Finding Severity
- `info` - Informational finding
- `warn` - Warning (non-blocking)
- `error` - Error (blocking)

These enums are **frozen** and must not change.

## Usage

### Validating a Report

```bash
# Using jsonschema CLI
jsonschema -i report.json contracts/schemas/sensor.report.v1.json

# Using xtask
cargo xtask conform --artifacts-dir artifacts/buildfix
```

### Generating Conformant Output

Tools should:
1. Set `schema` to their versioned schema identifier
2. Always include `tool.name`, `tool.version`, `run.started_at`
3. Use only frozen vocabulary values
4. Include `capabilities` block for "No Green By Omission"

## Compatibility

- Adding optional fields is backwards-compatible
- Removing required fields is a breaking change
- Changing enum values is a breaking change

All breaking changes require a version bump.
