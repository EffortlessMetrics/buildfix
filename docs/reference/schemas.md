# Output Schemas

Reference for buildfix output file formats.

## Overview

buildfix produces these artifacts:

| File | Schema | Description |
|------|--------|-------------|
| `plan.json` | buildfix.plan.v1 | Planned operations |
| `apply.json` | buildfix.apply.v1 | Execution results |
| `report.json` | buildfix.report.v1 | Cockpit receipt envelope |
| `plan.md` | — | Human-readable plan |
| `apply.md` | — | Human-readable apply result |
| `patch.diff` | — | Unified diff |

JSON schemas are in the `schemas/` directory.

## plan.json

Schema: `buildfix.plan.v1`

### Structure

```json
{
  "schema": "buildfix.plan.v1",
  "plan_id": "uuid-v4",
  "tool": {
    "name": "buildfix",
    "version": "0.1.0",
    "repo": null,
    "commit": null
  },
  "created_at": "2024-01-15T10:30:00Z",
  "config": {
    "allow": [],
    "deny": [],
    "require_clean_hashes": true
  },
  "receipts_consumed": [
    {
      "path": "artifacts/builddiag/report.json",
      "tool": "builddiag",
      "findings_count": 2
    }
  ],
  "preconditions": {
    "files": {
      "Cargo.toml": "sha256:abc123..."
    },
    "git_head": "def456...",
    "dirty": false
  },
  "fixes": [
    {
      "fix_id": "cargo.workspace_resolver_v2",
      "trigger": {
        "sensor": "builddiag",
        "check_id": "workspace.resolver_v2",
        "code": null
      },
      "target_file": "Cargo.toml",
      "operation": {
        "type": "EnsureWorkspaceResolverV2",
        "file": "Cargo.toml"
      },
      "safety": "safe",
      "rationale": "...",
      "blocked": false,
      "block_reason": null
    }
  ],
  "summary": {
    "fixes_total": 1,
    "safe": 1,
    "guarded": 0,
    "unsafe_": 0,
    "blocked": 0
  }
}
```

### Fields

#### Root

| Field | Type | Description |
|-------|------|-------------|
| `schema` | string | Schema identifier |
| `plan_id` | string | Unique plan identifier (UUID v4) |
| `tool` | ToolInfo | Tool metadata |
| `created_at` | string | ISO 8601 timestamp |
| `config` | Config | Policy snapshot |
| `receipts_consumed` | ReceiptRef[] | Input receipts |
| `preconditions` | Preconditions | File hashes and git state |
| `fixes` | PlannedFix[] | Planned operations |
| `summary` | Summary | Counts by safety class |

#### PlannedFix

| Field | Type | Description |
|-------|------|-------------|
| `fix_id` | string | Fix identifier |
| `trigger` | TriggerKey | Sensor finding that triggered fix |
| `target_file` | string | File to modify |
| `operation` | Operation | Edit operation |
| `safety` | string | `safe`, `guarded`, or `unsafe` |
| `rationale` | string | Explanation |
| `blocked` | bool | Whether fix is blocked |
| `block_reason` | string? | Why blocked |

#### Operation

Tagged union by `type`:

```json
// EnsureWorkspaceResolverV2
{"type": "EnsureWorkspaceResolverV2", "file": "Cargo.toml"}

// EnsurePathDepHasVersion
{"type": "EnsurePathDepHasVersion", "file": "Cargo.toml", "dep_name": "foo", "version": "1.0.0"}

// UseWorkspaceDependency
{"type": "UseWorkspaceDependency", "file": "crates/bar/Cargo.toml", "dep_name": "serde", "preserve_keys": ["features"]}

// NormalizeRustVersion
{"type": "NormalizeRustVersion", "file": "crates/bar/Cargo.toml", "rust_version": "1.70"}
```

## apply.json

Schema: `buildfix.apply.v1`

### Structure

```json
{
  "schema": "buildfix.apply.v1",
  "plan_id": "uuid-v4",
  "tool": {
    "name": "buildfix",
    "version": "0.1.0"
  },
  "applied_at": "2024-01-15T10:35:00Z",
  "dry_run": false,
  "preconditions_verified": true,
  "results": [
    {
      "fix_id": "cargo.workspace_resolver_v2",
      "status": "applied",
      "file": "Cargo.toml",
      "before_hash": "sha256:abc123...",
      "after_hash": "sha256:def456...",
      "backup_path": "artifacts/buildfix/backups/Cargo.toml.buildfix.bak",
      "error": null
    }
  ],
  "summary": {
    "attempted": 1,
    "applied": 1,
    "skipped": 0,
    "failed": 0
  }
}
```

### Fields

#### Root

| Field | Type | Description |
|-------|------|-------------|
| `schema` | string | Schema identifier |
| `plan_id` | string | Plan this apply executed |
| `tool` | ToolInfo | Tool metadata |
| `applied_at` | string | ISO 8601 timestamp |
| `dry_run` | bool | Whether this was a dry run |
| `preconditions_verified` | bool | All hashes matched |
| `results` | ApplyResult[] | Per-fix results |
| `summary` | ApplySummary | Counts |

#### ApplyResult

| Field | Type | Description |
|-------|------|-------------|
| `fix_id` | string | Fix identifier |
| `status` | string | `applied`, `skipped`, or `failed` |
| `file` | string | Target file |
| `before_hash` | string? | SHA256 before edit |
| `after_hash` | string? | SHA256 after edit |
| `backup_path` | string? | Backup file location |
| `error` | string? | Error message if failed |

## report.json

Schema: `buildfix.report.v1`

Cockpit-compatible receipt envelope for integration with the director system.

### Structure

```json
{
  "schema": "buildfix.report.v1",
  "tool": {
    "name": "buildfix",
    "version": "0.1.0"
  },
  "run": {
    "started_at": "2024-01-15T10:30:00Z",
    "ended_at": "2024-01-15T10:30:05Z",
    "git_head_sha": null
  },
  "verdict": {
    "status": "pass",
    "counts": {
      "findings": 1,
      "errors": 0,
      "warnings": 0
    },
    "reasons": []
  },
  "findings": [],
  "data": {
    "plan_id": "uuid-v4",
    "fixes_total": 1,
    "safe": 1,
    "guarded": 0,
    "unsafe": 0
  }
}
```

### Verdict Status

| Status | Meaning |
|--------|---------|
| `pass` | Plan created or apply succeeded |
| `warn` | Fixes available but not applied |
| `fail` | Apply failed |

## Markdown Files

### plan.md

Human-readable plan summary:

```markdown
# buildfix Plan

**Plan ID**: abc123...
**Created**: 2024-01-15T10:30:00Z

## Summary

| Category | Count |
|----------|-------|
| Total fixes | 2 |
| Safe | 1 |
| Guarded | 1 |
| Unsafe | 0 |
| Blocked | 0 |

## Planned Fixes

### 1. cargo.workspace_resolver_v2 (Safe)

**File**: Cargo.toml
**Operation**: EnsureWorkspaceResolverV2

Triggered by: builddiag / workspace.resolver_v2

---
...
```

### apply.md

Human-readable apply result:

```markdown
# buildfix Apply

**Plan ID**: abc123...
**Applied**: 2024-01-15T10:35:00Z
**Mode**: Live (not dry-run)

## Summary

| Status | Count |
|--------|-------|
| Applied | 1 |
| Skipped | 0 |
| Failed | 0 |

## Results

### cargo.workspace_resolver_v2: Applied

**File**: Cargo.toml
**Backup**: artifacts/buildfix/backups/Cargo.toml.buildfix.bak

---
...
```

## patch.diff

Standard unified diff format:

```diff
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -1,5 +1,6 @@
 [workspace]
 members = ["crates/*"]
+resolver = "2"

 [workspace.package]
 version = "0.1.0"
```

## See Also

- [CLI Reference](cli.md)
- [Exit Codes](exit-codes.md)
