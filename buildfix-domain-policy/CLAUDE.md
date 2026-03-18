# CLAUDE.md

Reusable policy and op-determinism helpers for buildfix planning.

## Build & Test

```bash
cargo build -p buildfix-domain-policy
cargo test -p buildfix-domain-policy
```

## Description

Applies planner-level policy: deterministic ordering, op-id generation, parameter filling, allow/deny filtering, and cap enforcement.

## Key Functions

- `apply_plan_policy()` — main entry point combining all policy passes
- `apply_params()` — fill user-provided parameters into ops requiring them
- `apply_allow_deny()` — apply allowlist/denylist policy gates
- `enforce_caps()` — block all ops when max_ops/max_files exceeded
- `deterministic_op_id()` — generate stable UUIDs from fix key + target + rule
- `args_fingerprint()` — SHA-256 fingerprint of JSON arguments
- `stable_op_sort_key()` — deterministic sort key for stable output
- `glob_match()` — lightweight wildcard matcher (* and ?) for policy keys

## Special Considerations

- Uses UUID v5 with a fixed namespace for deterministic IDs
- Canonicalizes JSON (sorts keys) before fingerprinting for consistency
- Caps block ALL operations when exceeded rather than truncating
- Policy keys support glob patterns: `*` and `?`
