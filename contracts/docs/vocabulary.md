# Frozen Vocabulary

This document defines the frozen enum values used across the Cockpit ecosystem. These values are **locked** and must not be changed.

## Verdict Status

The overall verdict status for a sensor or director report.

| Value | Description | Exit Code |
|-------|-------------|-----------|
| `pass` | All checks passed, no issues found | 0 |
| `warn` | Non-blocking issues found | 0 |
| `fail` | Blocking issues found | 2 |
| `skip` | Sensor did not run (preconditions unmet) | 0 |

**JSON Schema:**
```json
{
  "type": "string",
  "enum": ["pass", "warn", "fail", "skip"]
}
```

### Status Hierarchy

When aggregating multiple results:
```
fail > warn > skip > pass
```

A director's status is the "worst" status of its sensors.

### When to Use Each Status

**pass:**
- All checks completed successfully
- No findings at warn or error level
- Normal, healthy state

**warn:**
- Checks completed but found non-blocking issues
- User attention recommended but not required
- CI should not fail on warn (by default)

**fail:**
- Blocking issues that must be addressed
- CI should fail on this status
- Requires user action before proceeding

**skip:**
- Sensor preconditions not met
- Example: No Cargo.toml found, so builddiag skipped
- Not an error - explicit acknowledgment that checks didn't run

## Finding Severity

The severity level of an individual finding.

| Value | Description | Blocking |
|-------|-------------|----------|
| `info` | Informational, no action needed | No |
| `warn` | Warning, action recommended | No |
| `error` | Error, action required | Yes |

**JSON Schema:**
```json
{
  "type": "string",
  "enum": ["info", "warn", "error"]
}
```

### Severity to Status Mapping

| Max Finding Severity | Verdict Status |
|---------------------|----------------|
| None | `pass` |
| `info` only | `pass` |
| `warn` (no `error`) | `warn` |
| Any `error` | `fail` |

## Exit Codes

Standardized exit codes for all ecosystem tools:

| Code | Meaning |
|------|---------|
| 0 | Success (pass, warn, or skip) |
| 1 | Tool error (I/O failure, parse error, crash) |
| 2 | Policy block (fail status, precondition mismatch, denied fix) |

## Safety Class (buildfix-specific)

Classification for repair operations:

| Value | Description | Auto-Apply |
|-------|-------------|------------|
| `safe` | Fully determined from repo, low risk | Yes |
| `guarded` | Deterministic but higher impact | With `--allow-guarded` |
| `unsafe` | Requires user parameters | With `--allow-unsafe` |

This enum is internal to buildfix and not part of the ecosystem contract.

## Compatibility Notes

### Adding Values

New enum values can only be added if:
1. Existing consumers can safely ignore them
2. A version bump accompanies the change
3. Documentation is updated

### Removing Values

Enum values should never be removed. They may be deprecated but must remain valid indefinitely.

### Renaming Values

Enum values must never be renamed. To "rename":
1. Add the new value
2. Emit both old and new during transition period
3. Eventually stop emitting old (but still accept it)
