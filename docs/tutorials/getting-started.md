# Getting Started with buildfix

This tutorial walks you through installing buildfix and generating your first repair plan.

## Prerequisites

- Rust toolchain (1.70+)
- A Cargo workspace with existing sensor receipts in `artifacts/*/report.json`

## Installation

Build from source:

```bash
git clone <repo-url>
cd buildfix
cargo build --release
```

The binary is at `target/release/buildfix`.

## Your First Plan

### 1. Check for receipts

buildfix reads sensor outputs from `artifacts/*/report.json`. Verify you have receipts:

```bash
ls artifacts/*/report.json
```

You should see files like:
- `artifacts/builddiag/report.json`
- `artifacts/depguard/report.json`

If you don't have receipts, run your sensors first (buildscan, builddiag, depguard).

### 2. Generate a plan

```bash
buildfix plan
```

This produces:
- `artifacts/buildfix/plan.json` — Machine-readable plan
- `artifacts/buildfix/plan.md` — Human-readable summary
- `artifacts/buildfix/patch.diff` — Preview of changes
- `artifacts/buildfix/report.json` — Cockpit-compatible receipt

### 3. Review the plan

Open `artifacts/buildfix/plan.md` to see what buildfix found:

```bash
cat artifacts/buildfix/plan.md
```

You'll see a summary like:

```
# buildfix Plan

## Summary
- Fixes: 3
- Safe: 2
- Guarded: 1
- Unsafe: 0

## Planned Fixes
...
```

### 4. Preview the patch

Check what would change:

```bash
cat artifacts/buildfix/patch.diff
```

This shows a unified diff of all planned edits.

## Understanding Safety Classes

Each fix has a safety classification:

| Class | Meaning | Apply behavior |
|-------|---------|---------------|
| **Safe** | Fully determined, low impact | Applied with `--apply` |
| **Guarded** | Deterministic but higher impact | Requires `--allow-guarded` |
| **Unsafe** | Needs user parameters | Requires `--allow-unsafe` + params |

## What's Next?

- [Your First Fix](first-fix.md) — Walk through applying a fix
- [Configure buildfix](../how-to/configure.md) — Customize policy with buildfix.toml
- [Fix Catalog](../reference/fixes.md) — See all available fixes
