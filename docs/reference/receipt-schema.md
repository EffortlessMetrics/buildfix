# Receipt Schema

> **Version**: 1.0  
> **Last Updated**: 2026-03-16

This document describes the receipt format that buildfix expects from sensor tools. Receipts are JSON files placed in `artifacts/<sensor-id>/report.json`.

---

## Overview

buildfix uses a **tolerant** receipt model:
- Unknown fields are ignored
- Optional fields may be absent
- The schema version identifies the format

This allows buildfix to work with sensor outputs as found, even if they contain extra metadata.

---

## Directory Structure

```
artifacts/
  <sensor-id>/
    report.json
  another-sensor/
    report.json
```

- `<sensor-id>`: Directory name becomes the `sensor_id` in receipts
- `report.json`: The receipt file

---

## Receipt Envelope

```json
{
  "schema": "buildfix/receipt.v1",
  "tool": {
    "name": "cargo-deny",
    "version": "0.18.0",
    "repo": "https://github.com/EmbarkStudios/cargo-deny",
    "commit": "abc123"
  },
  "run": {
    "started_at": "2024-01-15T10:30:00Z",
    "ended_at": "2024-01-15T10:30:05Z",
    "git_head_sha": "def456"
  },
  "verdict": {
    "status": "fail",
    "counts": {
      "findings": 5,
      "errors": 3,
      "warnings": 2
    },
    "reasons": ["missing license", "unlicensed crate"]
  },
  "capabilities": {
    "check_ids": [
      "licenses.unlicensed",
      "licenses.missing_license",
      "bans.unused"
    ],
    "scopes": ["workspace", "crate"],
    "partial": false
  },
  "findings": [
    {
      "severity": "error",
      "check_id": "licenses.unlicensed",
      "code": "SPDX-0",
      "message": "Failed to find a valid SPDX license expression",
      "location": {
        "path": "crates/foo/Cargo.toml",
        "line": 5,
        "column": 1
      },
      "fingerprint": "abc123def456",
      "data": {}
    }
  ],
  "data": {}
}
```

---

## Schema Versions

| Version | Description |
|---------|-------------|
| `buildfix/receipt.v1` | Current format |

---

## Field Reference

### `schema` (required)

String identifying the receipt schema format.

**Example**: `"buildfix/receipt.v1"`

---

### `tool` (required)

Information about the sensor tool.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Tool name (e.g., "cargo-deny") |
| `version` | string | No | Tool version |
| `repo` | string | No | Repository URL |
| `commit` | string | No | Git commit SHA |

---

### `run` (optional)

Information about when the sensor ran.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `started_at` | ISO8601 | No | Run start time |
| `ended_at` | ISO8601 | No | Run end time |
| `git_head_sha` | string | No | Git HEAD SHA at run time |

---

### `verdict` (optional)

Summary of the sensor's findings.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `status` | enum | No | `pass`, `warn`, `fail`, `unknown` |
| `counts.findings` | u64 | No | Total findings |
| `counts.errors` | u64 | No | Error count |
| `counts.warnings` | u64 | No | Warning count |
| `reasons` | array | No | List of reason strings |

---

### `capabilities` (optional)

"No Green By Omission" pattern — declares what the sensor can check.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `check_ids` | array | No | List of check IDs this sensor can emit |
| `scopes` | array | No | Scopes covered (e.g., "workspace", "crate") |
| `partial` | bool | No | True if some inputs couldn't be processed |
| `reason` | string | No | Explanation for partial results |

---

### `findings` (optional)

List of individual findings from the sensor.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `severity` | enum | No | `info`, `warn`, `error` |
| `check_id` | string | No | Identifier for the check |
| `code` | string | No | Specific error/warning code |
| `message` | string | No | Human-readable message |
| `location.path` | string | No | File path (relative or absolute) |
| `location.line` | u64 | No | Line number |
| `location.column` | u64 | No | Column number |
| `fingerprint` | string | No | Stable key for deduplication |
| `data` | object | No | Tool-specific payload |

---

### `data` (optional)

Tool-specific arbitrary payload.

---

## Versioning Strategy

1. **Minor additions**: New optional fields can be added without version bump
2. **Breaking changes**: New schema version (e.g., `v2`)

Sensors should:
- Write valid JSON
- Include `schema` field
- Provide `tool.name` for traceability

---

## Adapter Integration

See [buildfix-adapter-sdk](../crate/buildfix_adapter_sdk) for the SDK that helps create adapters.

---

## See Also

- [CLI Reference](cli.md)
- [Configuration](config.md)
- [Troubleshooting](../how-to/troubleshoot.md)
