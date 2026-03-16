# Determinism: Why Byte-Stable Outputs Matter

This document explains buildfix's determinism guarantees and why they're essential.

## The Guarantee

Given the same inputs:
- Same receipts
- Same repo state
- Same config and params

buildfix produces **byte-identical** outputs:
- Same `plan.json`
- Same `patch.diff`
- Same ordering of ops

## Why Determinism Matters

Determinism enables reproducible CI runs, trustworthy reviews, and reliable auditing. If plans or patches reorder or drift, users can't trust the output.

## How buildfix Achieves Determinism

### 1. Stable Sorting

Ops are sorted deterministically:

```rust
ops.sort_by_key(stable_op_sort_key);
```

The sort key includes:
- Policy key (`sensor/check_id/code`)
- Target path
- Rule id and args fingerprint

### 2. Deterministic Op IDs

Op IDs are UUID v5 values derived from stable inputs:

```
{fix_key}|{target}|{rule_id}|{args_hash}
```

This keeps IDs stable across runs when inputs are unchanged.

### 3. No Timing Dependencies in Plan/Apply

Plan and apply artifacts do not embed wall-clock timestamps. Report envelopes do include timestamps, but they are separate from planning and apply outputs.

### 4. Normalized Paths

All paths are normalized:
- Repo-relative
- Forward slashes (even on Windows)
- No leading `./`

## Testing Determinism

### Golden Fixtures

Fixtures capture expected outputs for known inputs and compare `plan.json`, `plan.md`, and `patch.diff` byte-for-byte.

### Property Testing

Proptest verifies invariants such as deterministic ordering and idempotency of ops.

### Multiple Runs

```bash
buildfix plan --out-dir /tmp/plan1
buildfix plan --out-dir /tmp/plan2
diff /tmp/plan1/plan.json /tmp/plan2/plan.json
```

## Acceptable Variations

If the repo changes, these fields will differ because they are derived from repo state:
- `repo.head_sha`
- `preconditions.files[*].sha256`

## See Also

- [Preconditions](preconditions.md)
- [Design Goals](../design.md)
- [Testing Strategy](../testing.md)
