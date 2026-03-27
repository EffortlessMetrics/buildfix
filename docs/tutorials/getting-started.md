# Getting Started with buildfix

This tutorial is for operators who already have sensor receipts and want buildfix to explain and repair a Cargo workspace.

## Prerequisites

- Rust toolchain
- A Cargo workspace with `builddiag` or `depguard` receipts in `artifacts/*/report.json`
- One of the supported fix paths: `resolver-v2`, `path-dep-version`, `workspace-inheritance`, or `duplicate-deps`

## Installation

Install from crates.io or build from source:

```bash
cargo install buildfix --locked
```

If you want a concrete sandbox, use [`examples/demo`](../../examples/demo/README.md) or pick a profile from [`examples/profiles`](../../examples/profiles/README.md).

## Your First Plan

### 1. Check the receipts

buildfix reads sensor outputs from `artifacts/*/report.json`. Verify the receipts exist before planning:

```bash
ls artifacts/*/report.json
```

For the supported lane, look for `builddiag` and `depguard` receipts.

### 2. Generate a plan

```bash
buildfix plan
```

This produces:

- `artifacts/buildfix/plan.json`
- `artifacts/buildfix/plan.md`
- `artifacts/buildfix/patch.diff`
- `artifacts/buildfix/report.json`

### 3. Review the plan

Open `artifacts/buildfix/plan.md` to see what buildfix found and why it thinks the change is safe:

```bash
cat artifacts/buildfix/plan.md
```

You should see the supported lane called out in plain language, not just a list of internal fix IDs.

### 4. Preview the patch

Check what would change:

```bash
cat artifacts/buildfix/patch.diff
```

The patch should only include edits you can explain from the receipts.

## Understanding Safety Classes

Each op has a safety classification:

| Class | Meaning | Apply behavior |
|-------|---------|---------------|
| **Safe** | Determined from repo truth | Applied with `--apply` |
| **Guarded** | Deterministic but higher impact | Requires `--allow-guarded` |
| **Unsafe** | Needs explicit parameters | Requires `--allow-unsafe` + params |

## What's Next?

- [Your First Fix](first-fix.md)
- [Configure buildfix](../how-to/configure.md)
- [Fix Catalog](../reference/fixes.md)
- [buildfix demo](../../examples/demo/README.md)
