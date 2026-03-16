# Artifact Layout

This document describes the filesystem conventions for Cockpit ecosystem artifacts.

## Directory Structure

```
<repo_root>/
└── artifacts/
    ├── buildscan/
    │   └── report.json
    ├── builddiag/
    │   └── report.json
    ├── depguard/
    │   └── report.json
    ├── buildfix/
    │   ├── report.json      # Sensor envelope (sensor.report.v1)
    │   ├── plan.json        # Repair plan (buildfix.plan.v1)
    │   ├── apply.json       # Apply results (buildfix.apply.v1)
    │   ├── plan.md          # Human-readable plan
    │   ├── apply.md         # Human-readable apply results
    │   └── patch.diff       # Unified diff preview
    └── cockpit/
        └── report.json      # Director aggregate (cockpit.report.v1)
```

## Path Conventions

### Repository-Relative Paths

All paths in reports and findings must be:
- Relative to the repository root
- Use forward slashes (`/`) as separator (even on Windows)
- No leading `./` prefix
- No trailing slashes

**Correct:**
```json
{
  "path": "crates/foo/Cargo.toml",
  "line": 5
}
```

**Incorrect:**
```json
{
  "path": "./crates/foo/Cargo.toml",
  "path": "crates\\foo\\Cargo.toml",
  "path": "/absolute/path/to/file"
}
```

### Artifact References

References between artifacts use relative paths from the report location:

```json
{
  "artifacts": {
    "plan": "plan.json",
    "apply": "apply.json",
    "patch": "patch.diff"
  }
}
```

## Sensor Output Location

Each sensor writes to `artifacts/<sensor_name>/report.json`:

| Sensor | Output Path |
|--------|-------------|
| buildscan | `artifacts/buildscan/report.json` |
| builddiag | `artifacts/builddiag/report.json` |
| depguard | `artifacts/depguard/report.json` |
| buildfix | `artifacts/buildfix/report.json` |

## Director Output

The director (cockpit) aggregates sensor results to:
- `artifacts/cockpit/report.json`

## CI Integration

For CI systems, artifacts are typically collected from:
```
artifacts/**/*.json
artifacts/**/*.md
artifacts/**/*.diff
```

## Backup Files

When `--apply` modifies files, backups are written to:
```
artifacts/buildfix/backups/<path>.buildfix.bak
```

The backup suffix is configurable via `buildfix.toml`.
