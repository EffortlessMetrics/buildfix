# How to Troubleshoot Blocked Fixes

This guide helps you debug why buildfix ops aren't being applied.

## Check the Apply Report

First, read the apply output:

```bash
cat artifacts/buildfix/apply.md
```

This shows which ops were:
- **Applied**: Successfully written
- **Blocked**: Denied by policy or preconditions
- **Skipped**: Dry-run only (not written)
- **Failed**: Error during apply

## Common Issues

### Exit Code 2: Policy Block

Exit code 2 means buildfix refused to apply for policy reasons. Check these causes:

#### Precondition Mismatch

The repo changed between plan and apply.

**Symptoms**:
```
Precondition failed: Cargo.toml hash mismatch
Expected: abc123...
Actual:   def456...
```

**Solution**: Re-run the plan:
```bash
buildfix plan
buildfix apply --apply
```

#### Dirty Working Tree

Uncommitted changes exist.

**Symptoms**:
```
Working tree is dirty. Use --allow-dirty to override.
```

**Solution**:
```bash
# Option 1: Commit or stash changes
git stash

# Option 2: Allow dirty tree (not recommended)
buildfix apply --apply --allow-dirty
```

#### Guarded Op Blocked

A guarded fix requires explicit approval.

**Symptoms**:
```
Op blocked: guarded (use --allow-guarded)
```

**Solution**:
```bash
buildfix apply --apply --allow-guarded
```

Or enable in config:
```toml
[policy]
allow_guarded = true
```

#### Unsafe Op Blocked

An unsafe fix needs parameters.

**Symptoms**:
```
Op blocked: unsafe (requires --allow-unsafe and parameters)
```

**Solution**: Provide required parameters:
```bash
buildfix apply --apply --allow-unsafe --param rust_version=1.75
```

Or in config:
```toml
[policy]
allow_unsafe = true

[params]
rust_version = "1.75"
```

#### Op Denied by Policy

The op is in your deny list or not in your allow list.

**Symptoms**:
```
Op depguard/deps.path_requires_version/missing_version denied by policy
```

**Solution**: Check `buildfix.toml`:
```toml
[policy]
# Remove from deny list
deny = []

# Or add to allow list (if non-empty)
allow = [
  "depguard/deps.path_requires_version/*",
]
```

#### Caps Exceeded

Too many operations, files, or patch size.

**Symptoms**:
```
Plan exceeds max_ops (50)
```

**Solution**: Increase limits or fix in batches:
```toml
[policy]
max_ops = 100
max_files = 50
max_patch_bytes = 500000
```

### Exit Code 1: Tool Error

Exit code 1 indicates a runtime error. Common causes:

#### Missing Receipts

No sensor outputs found.

**Symptoms**:
```
No receipts found in artifacts/
```

**Solution**: Run sensors first:
```bash
cargo run -p builddiag
cargo run -p depguard
```

#### Invalid Plan

The plan.json is corrupted or incompatible.

**Symptoms**:
```
Failed to parse plan.json: ...
```

**Solution**: Regenerate the plan:
```bash
buildfix plan
```

#### Unparseable TOML

A Cargo.toml file has syntax errors.

**Symptoms**:
```
Failed to parse Cargo.toml: TOML parse error at line 42
```

**Solution**: Fix the TOML syntax in the reported file.

## Explain a Fix

Use `buildfix explain` to understand what a fix does and why it might be blocked:

```bash
buildfix explain resolver-v2
buildfix explain path-dep-version
buildfix explain workspace-inheritance
buildfix explain msrv
```

This shows:
- What the fix does
- Safety classification and rationale
- Triggering sensor findings
- Manual remediation steps

## Enable Debug Logging

Get verbose output with the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug buildfix plan
RUST_LOG=debug buildfix apply --apply
```

This shows:
- Config loading and merging
- Receipt discovery
- Fixer routing
- Precondition verification

## Inspect the Plan

Check what buildfix planned to do:

```bash
# Human-readable summary
cat artifacts/buildfix/plan.md

# Machine-readable details
cat artifacts/buildfix/plan.json | jq '.ops'

# Patch preview
cat artifacts/buildfix/patch.diff
```

## Verify Preconditions Manually

Check that files haven't changed:

```bash
# Get expected hashes from plan
cat artifacts/buildfix/plan.json | jq '.preconditions.files'

# Compare with actual
sha256sum Cargo.toml
```

## Reset and Retry

If things are in a bad state:

```bash
# Remove buildfix artifacts
rm -rf artifacts/buildfix/

# Restore from backups (if apply partially ran)
cp artifacts/buildfix/backups/Cargo.toml.buildfix.bak Cargo.toml

# Start fresh
buildfix plan
buildfix apply --apply
```

## See Also

- [Exit Codes](../reference/exit-codes.md)
- [Safety Model](../safety-model.md)
- [Configuration](configure.md)
