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

JSON schemas are in the `schemas/` directory and embedded in the CLI.

## plan.json

Schema: `buildfix.plan.v1`

### Structure

```json
{
  "schema": "buildfix.plan.v1",
  "tool": { "name": "buildfix", "version": "0.1.0" },
  "repo": { "root": "/repo", "head_sha": "...", "dirty": false },
  "inputs": [
    {
      "path": "artifacts/builddiag/report.json",
      "schema": "builddiag.report.v1",
      "tool": "builddiag"
    }
  ],
  "policy": {
    "allow": [],
    "deny": [],
    "allow_guarded": false,
    "allow_unsafe": false,
    "allow_dirty": false,
    "max_ops": 50,
    "max_files": 25,
    "max_patch_bytes": 250000
  },
  "preconditions": {
    "files": [
      { "path": "Cargo.toml", "sha256": "<sha256>" }
    ],
    "head_sha": "...",
    "dirty": false
  },
  "ops": [
    {
      "id": "<uuid-v5>",
      "safety": "safe",
      "blocked": false,
      "target": { "path": "Cargo.toml" },
      "kind": {
        "type": "toml_transform",
        "rule_id": "ensure_workspace_resolver_v2",
        "args": null
      },
      "rationale": {
        "fix_key": "builddiag/workspace.resolver_v2/not_v2",
        "description": "Adds resolver = \"2\" to workspace",
        "findings": [
          {
            "source": "builddiag",
            "check_id": "workspace.resolver_v2",
            "code": "not_v2",
            "path": "Cargo.toml",
            "line": 1
          }
        ]
      },
      "params_required": [],
      "preview": { "patch_fragment": "@@ ..." }
    }
  ],
  "summary": {
    "ops_total": 1,
    "ops_blocked": 0,
    "files_touched": 1,
    "patch_bytes": 42
  }
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `schema` | string | Schema identifier (`buildfix.plan.v1`) |
| `tool` | object | Tool metadata (`name`, `version`, optional `commit`) |
| `repo` | object | Repository info (`root`, optional `head_sha`, `dirty`) |
| `inputs` | array | Receipt inputs used to plan |
| `policy` | object | Policy snapshot (allow/deny, safety flags, caps) |
| `preconditions` | object | File SHA256 and optional git state checks |
| `ops` | array | Planned operations (op-level) |
| `summary` | object | Counts and patch size |

### op

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Deterministic op ID (UUID v5) |
| `safety` | string | `safe`, `guarded`, or `unsafe` |
| `blocked` | bool | Whether this op is blocked by policy |
| `blocked_reason` | string? | Why blocked (allow/deny, caps, missing params) |
| `target` | object | Target file path (`path`) |
| `kind` | object | Operation kind (see below) |
| `rationale` | object | `fix_key`, description, and findings |
| `params_required` | string[] | Required parameters for unsafe ops |
| `preview` | object? | Optional patch fragment preview |

### op.kind

`op.kind` is a tagged object with `type`:

- `toml_set` with `toml_path` and `value`
- `toml_remove` with `toml_path`
- `toml_transform` with `rule_id` and optional `args`

## apply.json

Schema: `buildfix.apply.v1`

### Structure

```json
{
  "schema": "buildfix.apply.v1",
  "tool": { "name": "buildfix", "version": "0.1.0" },
  "repo": {
    "root": "/repo",
    "head_sha_before": "...",
    "head_sha_after": "...",
    "dirty_before": false,
    "dirty_after": false
  },
  "plan_ref": { "path": "artifacts/buildfix/plan.json", "sha256": "..." },
  "preconditions": {
    "verified": true,
    "mismatches": []
  },
  "results": [
    {
      "op_id": "<uuid-v5>",
      "status": "applied",
      "message": null,
      "blocked_reason": null,
      "files": [
        {
          "path": "Cargo.toml",
          "sha256_before": "...",
          "sha256_after": "...",
          "backup_path": "artifacts/buildfix/backups/Cargo.toml.buildfix.bak"
        }
      ]
    }
  ],
  "summary": {
    "attempted": 1,
    "applied": 1,
    "blocked": 0,
    "failed": 0,
    "files_modified": 1
  }
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `schema` | string | Schema identifier (`buildfix.apply.v1`) |
| `tool` | object | Tool metadata (`name`, `version`, optional `commit`) |
| `repo` | object | Repo state before/after apply |
| `plan_ref` | object | Path and optional SHA256 of plan.json |
| `preconditions` | object | `verified` and any mismatches |
| `results` | array | Per-op results |
| `summary` | object | Apply counts |

### result

| Field | Type | Description |
|-------|------|-------------|
| `op_id` | string | Plan op ID |
| `status` | string | `applied`, `blocked`, `failed`, or `skipped` |
| `message` | string? | Optional message |
| `blocked_reason` | string? | Policy block reason |
| `files` | array | File-level hashes and backups |

## report.json

Schema: `buildfix.report.v1`

Cockpit-compatible receipt envelope for integration with the director system.

### Structure

```json
{
  "schema": "buildfix.report.v1",
  "tool": { "name": "buildfix", "version": "0.1.0" },
  "run": {
    "started_at": "2024-01-15T10:30:00Z",
    "ended_at": "2024-01-15T10:30:05Z",
    "duration_ms": 5000
  },
  "verdict": {
    "status": "warn",
    "counts": { "info": 0, "warn": 1, "error": 0 },
    "reasons": []
  },
  "capabilities": {
    "inputs_available": ["artifacts/builddiag/report.json"],
    "inputs_failed": []
  },
  "findings": [],
  "artifacts": {
    "plan": "artifacts/buildfix/plan.json",
    "apply": "artifacts/buildfix/apply.json",
    "patch": "artifacts/buildfix/patch.diff"
  },
  "data": {
    "ops_total": 1,
    "ops_blocked": 0
  }
}
```

### Capabilities Block

The `capabilities` block implements the "No Green By Omission" pattern. It explicitly tracks which sensor inputs were successfully loaded and which failed.

| Field | Type | Description |
|-------|------|-------------|
| `inputs_available` | string[] | Paths to successfully loaded receipts |
| `inputs_failed` | object[] | Receipts that failed to load |

Each `inputs_failed` entry contains:

| Field | Type | Description |
|-------|------|-------------|
| `path` | string | Path to the failed receipt |
| `reason` | string | Human-readable failure reason |

This ensures:
- A passing verdict with empty `inputs_available` signals a problem
- Failed inputs are explicitly tracked, not silently ignored
- Consumers can distinguish "no issues found" from "nothing was checked"

### Artifacts Block

The `artifacts` block contains paths to related output files:

| Field | Type | Description |
|-------|------|-------------|
| `plan` | string? | Path to plan.json if generated |
| `apply` | string? | Path to apply.json if generated |
| `patch` | string? | Path to patch.diff if generated |

### Verdict Status

| Status | Meaning |
|--------|---------|
| `pass` | Plan created or apply succeeded |
| `warn` | Ops available but not applied or blocked |
| `fail` | Apply failed |

## Markdown Files

### plan.md

Human-readable plan summary with ops, safety, blocked reasons, and findings.

### apply.md

Human-readable apply result with per-op status and file hashes.

## patch.diff

Standard unified diff format.

## See Also

- [CLI Reference](cli.md)
- [Exit Codes](exit-codes.md)
