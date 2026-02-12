# Preconditions: Drift Detection

This document explains how buildfix prevents "plan on one state, apply on another" errors through precondition verification.

## The Problem

Without preconditions, this dangerous scenario is possible:

1. Developer A runs `buildfix plan` on commit abc123
2. Developer B pushes changes to Cargo.toml
3. Developer A runs `buildfix apply` on commit def456
4. The plan was computed for a different file state
5. The edit produces unexpected or corrupted results

## The Solution

buildfix captures file state at plan time and verifies it at apply time.

### Plan Phase

When generating a plan, buildfix:

1. Computes SHA256 hashes of all files that will be modified
2. Captures the current git HEAD (if available)
3. Records whether the working tree is dirty
4. Stores this snapshot in `plan.json`

```json
{
  "preconditions": {
    "files": [
      { "path": "Cargo.toml", "sha256": "e3b0c44298fc1c14..." },
      { "path": "crates/foo/Cargo.toml", "sha256": "d7a8fbb307d78094..." }
    ],
    "head_sha": "abc123def456...",
    "dirty": false
  }
}
```

### Apply Phase

Before writing any changes, buildfix:

1. Loads the preconditions from `plan.json`
2. Computes fresh hashes of each target file
3. Compares against stored hashes
4. If any mismatch: **stops with exit code 2**

No changes are written if preconditions fail.

## Hash Algorithm

buildfix uses SHA256 for file hashes:

- Cryptographically strong
- Fast to compute
- Widely understood
- Produces stable, reproducible digests

The hash covers the entire file contents, byte-for-byte.

## What Triggers a Mismatch

Any change to a target file causes a mismatch:

| Change Type | Causes Mismatch |
|-------------|-----------------|
| Content edit | Yes |
| Whitespace change | Yes |
| Line ending change | Yes |
| File permission | No (content only) |
| File timestamp | No (content only) |

## Recovery from Mismatch

When preconditions fail:

```
Precondition failed: Cargo.toml hash mismatch
Expected: sha256:abc123...
Actual:   sha256:def456...
```

Resolution:

```bash
# Regenerate the plan for current state
buildfix plan

# Then apply
buildfix apply --apply
```

## Disabling Preconditions

Preconditions can be disabled (not recommended):

```bash
buildfix plan --no-clean-hashes
```

This clears `preconditions.files` in the plan. Apply will skip hash verification.

**Warning**: Disabling preconditions removes drift protection. Only do this if you understand the risks.

## Git Head Verification

In addition to file hashes, buildfix captures the git HEAD SHA:

```json
{
  "preconditions": {
    "head_sha": "abc123def456..."
  }
}
```

When provided (via `--git-head-precondition`), the head SHA is enforced and will block apply on mismatch.

## Dirty Tree Detection

By default, buildfix refuses to apply when the working tree is dirty:

```
Working tree is dirty. Use --allow-dirty to override.
```

This prevents:
- Mixing buildfix changes with uncommitted work
- Confusion about what changed
- Lost changes if something goes wrong

Override with `--allow-dirty` or in config:

```toml
[policy]
allow_dirty = true
```

The dirty flag is recorded in artifacts for auditability.

## Backup Strategy

Even with preconditions, buildfix creates backups before editing:

```
artifacts/buildfix/backups/
├── Cargo.toml.buildfix.bak
└── crates/foo/Cargo.toml.buildfix.bak
```

This provides:
- Recovery from unexpected issues
- Audit trail of original state
- Defense in depth

Backups are configurable:

```toml
[backups]
enabled = true
suffix = ".buildfix.bak"
```

## Design Rationale

### Why File Hashes Over Git Diff?

File hashes are preferred because:

1. **Simpler**: No git dependency for verification
2. **Faster**: Hash comparison is O(1)
3. **Reliable**: Works with any file state, staged or not
4. **Deterministic**: Same file always produces same hash

### Why Exit 2 Instead of Retrying?

When preconditions fail, buildfix exits rather than retrying because:

1. **Safety**: Automatic retry might apply outdated logic
2. **Clarity**: User must consciously regenerate the plan
3. **Audit**: Every plan-apply cycle is explicitly tracked
4. **Simplicity**: No complex retry/merge logic

### Why Capture at Plan Time?

Capturing preconditions at plan time (not apply time) ensures:

1. The plan is a complete "contract of intent"
2. Plans can be reviewed before apply
3. Plans can be stored and applied later
4. Drift is detectable across any time gap

## Backup Recovery

If an apply operation fails or produces unexpected results, you can recover using backup files.

### Locating Backups

Backups are stored in the artifacts directory:

```
artifacts/buildfix/backups/
├── Cargo.toml.buildfix.bak
└── crates/foo/Cargo.toml.buildfix.bak
```

The backup path mirrors the original file path with the `.buildfix.bak` suffix appended.

### Manual Recovery

To restore a single file:

```bash
# View the backup
cat artifacts/buildfix/backups/Cargo.toml.buildfix.bak

# Restore by copying
cp artifacts/buildfix/backups/Cargo.toml.buildfix.bak Cargo.toml
```

To restore all backups:

```bash
# Find and restore all backups
for bak in artifacts/buildfix/backups/**/*.buildfix.bak; do
  original="${bak#artifacts/buildfix/backups/}"
  original="${original%.buildfix.bak}"
  cp "$bak" "$original"
done
```

### Backup Information in Artifacts

The `apply.json` artifact records backup paths for each modified file:

```json
{
  "results": [
    {
      "op_id": "...",
      "status": "applied",
      "files": [
        {
          "path": "Cargo.toml",
          "sha256_before": "abc123...",
          "sha256_after": "def456...",
          "backup_path": "artifacts/buildfix/backups/Cargo.toml.buildfix.bak"
        }
      ]
    }
  ]
}
```

### Configuring Backups

Backup behavior is configurable in `buildfix.toml`:

```toml
[backups]
enabled = true              # Create backups before editing
suffix = ".buildfix.bak"    # Suffix appended to backup files
```

To disable backups (not recommended):

```toml
[backups]
enabled = false
```

### Best Practices

1. **Always keep backups enabled** for production use
2. **Commit before applying** so you can use `git checkout` as an alternative recovery
3. **Review apply.json** after apply to understand what changed
4. **Clean up old backups** periodically to avoid confusion

### When Backups Are Not Created

Backups are skipped when:
- `backups.enabled = false` in config
- Running in dry-run mode (`buildfix apply` without `--apply`)
- No files are actually modified (all ops blocked or skipped)

## See Also

- [Safety Model](../safety-model.md)
- [Design Goals](../design.md)
- [Determinism](determinism.md)
