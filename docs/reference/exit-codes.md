# Exit Codes

buildfix uses semantic exit codes to indicate outcome.

## Summary

| Code | Name | Meaning |
|------|------|---------|
| 0 | Success | Operation completed successfully |
| 1 | Error | Tool or runtime error |
| 2 | Policy Block | Policy-based refusal |

## Exit Code 0: Success

The operation completed successfully.

### Plan

- Receipts were loaded
- Plan was generated
- Artifacts were written

This includes cases where the plan is empty (no applicable ops).

### Apply

- Plan was loaded
- Preconditions were verified
- Changes were applied (or would be in dry-run)
- Artifacts were written

### Validate

- All receipts loaded successfully
- All artifacts validated against their schemas
- No validation errors found

### Explain

- Fix key was found and explanation displayed

### ListFixes

- Fix list was displayed

## Exit Code 1: Error

A tool or runtime error occurred. This indicates a problem with buildfix itself or its inputs.

### Common Causes

| Cause | Example Message |
|-------|-----------------|
| Missing receipts | `No receipts found in artifacts/` |
| Invalid TOML | `Failed to parse Cargo.toml: TOML parse error` |
| Invalid plan | `Failed to parse plan.json: expected field 'ops'` |
| I/O error | `Failed to write plan.json: permission denied` |
| Missing plan | `File not found: artifacts/buildfix/plan.json` |
| Unknown fix key | `Unknown fix key: 'foo'` (for explain command) |
| Invalid argument combo | `--auto-commit requires --apply` |
| Auto-commit failure | `auto-commit failed: git commit returned non-zero` |

### Debugging

Enable debug logging:

```bash
RUST_LOG=debug buildfix plan
```

## Exit Code 2: Policy Block

buildfix refused to proceed due to policy constraints. This is intentional behavior, not an error.

### Common Causes

#### Precondition Mismatch

Files changed between plan and apply.

```
Precondition failed: Cargo.toml hash mismatch
Expected: sha256:abc123...
Actual:   sha256:def456...
```

**Resolution**: Re-run `buildfix plan` to generate a fresh plan.

#### Dirty Working Tree

Uncommitted changes in the repository (default behavior).

```
Working tree is dirty. Use --allow-dirty to override.
```

**Resolution**:
- Commit or stash changes, or
- Use `--allow-dirty` (not recommended)

#### Guarded Op Blocked

A guarded fix requires explicit approval.

```
Op blocked: guarded ops require --allow-guarded
```

**Resolution**: Add `--allow-guarded` to the apply command.

#### Unsafe Op Blocked

An unsafe fix needs parameters or explicit approval.

```
Op blocked: unsafe ops require --allow-unsafe and parameters
```

**Resolution**: Provide required parameters via CLI or config.

#### Op Denied by Policy

The fix matches a deny pattern or isn't in the allow list.

```
Op depguard/deps.path_requires_version/missing_version denied by policy
```

**Resolution**: Update `buildfix.toml` allow/deny lists.

#### Caps Exceeded

Plan exceeds operational limits.

```
Plan exceeds max_ops limit (50)
```

```
Plan exceeds max_files limit (25)
```

```
Plan exceeds max_patch_bytes limit (250000)
```

**Resolution**: Increase limits in `buildfix.toml` or fix in batches.

#### Validation Failure

Schema validation failed for receipts or artifacts.

```
artifacts/buildfix/plan.json: property 'ops' is required
```

**Resolution**: Fix the invalid artifact or regenerate it.

### Blocked Reason Tokens

When an op is blocked, the `blocked_reason_token` field provides a machine-readable token:

| Token | Meaning |
|-------|---------|
| `missing_params` | Unsafe op missing required parameters |
| `denylist` | Op matched a deny pattern |
| `allowlist_missing` | Op not in allowlist |
| `max_ops` | Plan exceeds max_ops limit |
| `max_files` | Plan exceeds max_files limit |
| `max_patch_bytes` | Plan exceeds max_patch_bytes limit |
| `dirty_working_tree` | Working tree has uncommitted changes |
| `safety_guarded_not_allowed` | Guarded op requires --allow-guarded |
| `safety_unsafe_not_allowed` | Unsafe op requires --allow-unsafe |
| `precondition_mismatch` | SHA256 hash mismatch on file |

## CI/CD Integration

### Treat Policy Blocks as Warnings

For informational runs (e.g., PR checks):

```bash
buildfix plan || test $? -eq 2
```

### Fail on Policy Blocks

For enforcement runs:

```bash
buildfix apply --apply
# Exit 2 will fail the job
```

### Distinguish Errors from Blocks

```bash
buildfix plan
code=$?
if [ $code -eq 1 ]; then
  echo "Error: buildfix failed"
  exit 1
elif [ $code -eq 2 ]; then
  echo "Warning: policy block"
  # Continue or exit based on your policy
fi
```

## Cockpit Mode

In cockpit mode (`--mode cockpit`), policy blocks (exit 2) are mapped to exit 0. This is useful for integration with systems that treat any non-zero exit as a failure.

```bash
buildfix plan --mode cockpit  # Returns 0 even if policy blocked
```

## Artifacts on Exit 2

Even when exiting with code 2, buildfix writes artifacts:

### Plan (exit 2)

- `plan.json` with blocked ops marked
- `plan.md` explaining blocks
- `report.json` with `warn` status

### Apply (exit 2)

- `apply.json` with `blocked` or `failed` results
- `apply.md` explaining what happened
- `report.json` with `warn` or `fail` status

Always check artifacts for details on why a policy block occurred.

## CLI Help Text

Exit codes are also documented in the CLI long help. Run `buildfix --help` to see the summary.

## See Also

- [CLI Reference](cli.md)
- [Configuration](config.md)
- [Troubleshooting](../how-to/troubleshoot.md)
