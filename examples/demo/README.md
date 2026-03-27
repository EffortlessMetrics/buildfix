# buildfix demo: fixing a real Cargo workspace

This self-contained demo walks through buildfix diagnosing and repairing
three common Cargo workspace hygiene issues -- automatically, deterministically,
and safely.

## The scenario

`repo/` is a small workspace with three crates:

```
repo/
  Cargo.toml            # workspace root
  crates/
    core/Cargo.toml     # acme-core v0.3.0
    api/Cargo.toml      # acme-api v0.3.0 (depends on core + serde)
    cli/Cargo.toml      # acme-cli v0.3.0 (depends on core + api + serde)
```

The workspace has three issues that buildfix can fix:

### 1. Missing `resolver = "2"` (safe)

The workspace `Cargo.toml` does not set `resolver = "2"`. Without it,
feature unification may behave unexpectedly in edition 2021+ workspaces.

### 2. Duplicate dependency versions (safe)

`serde` appears in three crates at three different versions:
- `core`: `serde = "1.0.200"`
- `api`: `serde = { version = "1.0.180", features = ["derive"] }`
- `cli`: `serde = "1.0.190"`

buildfix consolidates these into `[workspace.dependencies]` with the
highest version (`1.0.200`), then rewrites each member to use
`serde = { workspace = true }` (preserving per-crate features like `derive`).

### 3. Path dependencies without version (safe)

`api/Cargo.toml` depends on `acme-core = { path = "../core" }` without a
`version` field. Same for `cli/Cargo.toml` depending on both `acme-core`
and `acme-api`. These deps are unpublishable to crates.io without a version.
buildfix reads the version from each dependency's own `Cargo.toml` and adds it.

## How it works

buildfix does not scan code itself. It reads **receipts** -- JSON reports
produced by sensor tools (linters, audit tools, dependency analyzers). The
`artifacts/` directory contains two pre-built sensor receipts:

- `artifacts/builddiag/report.json` -- reports the missing resolver v2
- `artifacts/depguard/report.json` -- reports duplicate deps and missing versions

From these receipts, buildfix generates a deterministic plan and a unified
patch.

## Running the demo

From the **buildfix repository root**:

```bash
# Step 1: Generate the repair plan
cargo run -p buildfix -- plan \
  --repo-root examples/demo/repo \
  --artifacts-dir examples/demo/artifacts \
  --out-dir examples/demo/output \
  --no-clean-hashes

# Step 2: Inspect what buildfix wants to do
cat examples/demo/output/plan.md
cat examples/demo/output/patch.diff

# Step 3: Apply the fixes (writes to disk)
cargo run -p buildfix -- apply \
  --repo-root examples/demo/repo \
  --out-dir examples/demo/output \
  --apply \
  --allow-dirty

# Step 4: See the repaired files
cat examples/demo/repo/Cargo.toml
cat examples/demo/repo/crates/api/Cargo.toml
cat examples/demo/repo/crates/cli/Cargo.toml
cat examples/demo/repo/crates/core/Cargo.toml
```

> **Tip:** After running, `git diff examples/demo/repo` shows exactly what
> changed. Use `git checkout -- examples/demo/repo` to reset the demo workspace
> back to its original state.

## The plan

Running `buildfix plan` produces 8 operations across 4 files, all classified
as **safe** (fully determined from repo truth, no user input needed):

```
# buildfix plan

- Ops: 8 (blocked 0)
- Files touched: 4
- Safety: 8 safe, 0 guarded, 0 unsafe
- Inputs: 2
```

## The patch

Here is what buildfix changes in each file:

**`Cargo.toml`** -- adds resolver v2 and a workspace-level serde dependency:

```diff
 [workspace]
 members = ["crates/api", "crates/core", "crates/cli"]
+resolver = "2"
+dependencies = { serde = "1.0.200" }
```

**`crates/core/Cargo.toml`** -- switches to workspace serde:

```diff
 [dependencies]
-serde = "1.0.200"
+serde = { workspace = true }
```

**`crates/api/Cargo.toml`** -- switches to workspace serde (keeping `derive`
feature) and adds version to path dep:

```diff
 [dependencies]
-acme-core = { path = "../core" }
-serde = { version = "1.0.180", features = ["derive"] }
+acme-core = { path = "../core" , version = "0.3.0" }
+serde = { workspace = true, features = ["derive"] }
```

**`crates/cli/Cargo.toml`** -- switches to workspace serde and adds versions
to path deps:

```diff
 [dependencies]
-acme-core = { path = "../core" }
-acme-api = { path = "../api" }
-serde = "1.0.190"
+acme-core = { path = "../core" , version = "0.3.0" }
+acme-api = { path = "../api" , version = "0.3.0" }
+serde = { workspace = true }
```

The full machine-readable patch is in `expected/patch.diff`.

## Safety model

Every operation in buildfix has a safety classification:

| Class | Meaning | Apply behavior |
|-------|---------|----------------|
| **safe** | Fully determined from repo truth | Auto-applied with `--apply` |
| **guarded** | Deterministic but higher impact | Requires `--apply --allow-guarded` |
| **unsafe** | Needs user-provided parameters | Plan-only; requires `--allow-unsafe` + `--param` |

All 8 operations in this demo are **safe** because:

- **resolver v2**: The workspace either has `resolver = "2"` or it does not.
  No ambiguity, no data to invent.
- **duplicate deps**: The highest version (`1.0.200`) is chosen as the
  consolidation target. Each member's per-crate features (like `derive`) are
  preserved.
- **path dep version**: The version is read directly from the dependency
  crate's own `Cargo.toml`. buildfix never guesses.

## Preconditions

buildfix includes SHA256 file hashes as preconditions in the plan. Before
applying any operation, it verifies the file on disk still matches the hash
from plan time. If someone edits a file between `plan` and `apply`, the
apply refuses to proceed (exit code 2) rather than silently overwriting.

This demo uses `--no-clean-hashes` to skip precondition hashing (since the
demo files are tracked within the buildfix repo itself, not in their own git
repository), but in production usage the precondition system prevents stale
plans from being applied.

## Expected outputs

The `expected/` directory contains reference copies of buildfix output:

| File | Description |
|------|-------------|
| `plan.json` | Full plan with all 8 operations and their rationale |
| `plan.md` | Human-readable plan summary |
| `patch.diff` | Unified diff of all changes |
| `comment.md` | PR comment summary |
| `report.json` | Sensor-envelope report (`sensor.report.v1`) |
| `extras/buildfix.report.v1.json` | Buildfix-specific report with plan stats |

> **Note:** Fields like `head_sha`, `started_at`, and `ended_at` in the
> expected files use placeholders (`<git-head-sha>`, `<timestamp>`) because
> they vary per machine and commit. The structural content (ops, patch, safety
> counts) is deterministic.

## What's next

- Run `cargo run -p buildfix -- list-fixes` to see all available fixers
- Run `cargo run -p buildfix -- explain resolver-v2` to understand a fixer's
  safety rationale
- See the [design docs](../../docs/design.md) for architecture details
