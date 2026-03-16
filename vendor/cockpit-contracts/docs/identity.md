# Identity: check_id and code Rules

This document defines the naming conventions for `check_id` and `code` fields in findings.

## Overview

Findings use a two-level identifier system:
- `check_id` - The category of check (e.g., `workspace_resolver`)
- `code` - The specific finding within that check (e.g., `missing_v2`)

Together they form a policy key: `<sensor>/<check_id>/<code>`

## Naming Rules

### check_id

**Pattern:** `^[a-z][a-z0-9_]*$`

- Lowercase letters, digits, and underscores only
- Must start with a letter
- Use underscores to separate words
- Should describe the category of check

**Examples:**
```
workspace_resolver
path_dep_version
msrv_consistent
edition_year
```

**Invalid:**
```
WorkspaceResolver   # No uppercase
workspace-resolver  # No hyphens
_workspace          # Must start with letter
123_check           # Must start with letter
```

### code

**Pattern:** `^[a-z][a-z0-9_]*$`

- Same rules as check_id
- Should describe the specific finding

**Examples:**
```
missing_v2
version_mismatch
deprecated
not_set
```

## Policy Keys

Policy keys combine sensor, check_id, and code for allowlist/denylist matching:

```
<sensor>/<check_id>/<code>
```

**Examples:**
```
builddiag/workspace_resolver/missing_v2
builddiag/path_dep_version/missing_version
depguard/license_check/copyleft_detected
```

### Glob Patterns

Policy configurations support glob patterns:

```toml
[policy]
allow = [
  "builddiag/workspace_resolver/*",
  "builddiag/edition_year/*",
]
deny = [
  "builddiag/msrv_consistent/*",
]
```

## Stability

Once a check_id or code is emitted by a sensor:
- It should not be renamed (would break user policies)
- It can be deprecated (emit both old and new)
- It can be removed in a major version bump

## Cross-Sensor Coordination

When multiple sensors can emit similar findings:
- Use consistent check_id names across sensors
- Document which sensor is authoritative
- Avoid duplicate findings for the same issue

## Examples by Sensor

### builddiag

| check_id | code | Description |
|----------|------|-------------|
| `workspace_resolver` | `missing_v2` | Workspace missing resolver v2 |
| `path_dep_version` | `missing_version` | Path dep lacks version |
| `edition_year` | `not_set` | Edition not specified |
| `msrv_consistent` | `mismatch` | MSRV inconsistent |

### depguard

| check_id | code | Description |
|----------|------|-------------|
| `license_check` | `copyleft_detected` | Copyleft license found |
| `security_advisory` | `vulnerability` | Known vulnerability |
| `outdated_dep` | `major_behind` | Major version behind |

### buildfix

| check_id | code | Description |
|----------|------|-------------|
| `fix_available` | `resolver_v2` | Fix available for resolver v2 |
| `fix_available` | `path_dep_version` | Fix available for path dep |
| `fix_blocked` | `policy_deny` | Fix blocked by policy |
