# Your First Fix

This tutorial walks through the complete plan → apply workflow, applying an op to your workspace.

## Scenario

You have a Cargo workspace missing `resolver = "2"` in the root `Cargo.toml`. The builddiag sensor flagged this, and buildfix can fix it automatically.

## Step 1: Generate the Plan

```bash
buildfix plan
```

Check what buildfix found:

```bash
cat artifacts/buildfix/plan.md
```

You should see the resolver-v2 op:

```
## Planned Ops

### 1. builddiag/workspace.resolver_v2/not_v2 (Safe)
File: Cargo.toml
Operation: toml_transform (ensure_workspace_resolver_v2)
```

## Step 2: Review the Patch

Preview the exact change:

```bash
cat artifacts/buildfix/patch.diff
```

```diff
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -1,5 +1,6 @@
 [workspace]
 members = ["crates/*"]
+resolver = "2"
```

## Step 3: Dry-Run Apply

Test the apply without writing changes:

```bash
buildfix apply
```

This validates preconditions and generates apply artifacts but doesn't modify files. Check `artifacts/buildfix/apply.md` for the result.

## Step 4: Apply for Real

When you're ready, apply the safe ops:

```bash
buildfix apply --apply
```

This:
1. Verifies file hashes match the plan (no drift since planning)
2. Creates backups in `artifacts/buildfix/backups/`
3. Writes the changes
4. Generates `apply.json` with the execution record

## Step 5: Verify

Check that the change was applied:

```bash
grep resolver Cargo.toml
```

You should see:

```toml
resolver = "2"
```

## Applying Guarded Fixes

Some ops require explicit approval. For example, MSRV normalization is guarded because changing rust-version affects compatibility.

To include guarded ops:

```bash
buildfix apply --apply --allow-guarded
```

## Handling Failures

If apply fails (exit code 2), check `artifacts/buildfix/apply.md` for details:

- **Precondition mismatch**: Files changed since plan was generated. Re-run `buildfix plan`.
- **Dirty working tree**: Commit or stash changes, or use `--allow-dirty`.
- **Policy block**: An op was denied by your buildfix.toml policy.

## What's Next?

- [Troubleshoot Blocked Fixes](../how-to/troubleshoot.md) — Debug why ops aren't applying
- [CLI Reference](../reference/cli.md) — All commands and options
- [Safety Model](../safety-model.md) — Understand safe/guarded/unsafe
