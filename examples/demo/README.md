# buildfix demo: fixing a real Cargo workspace

This demo shows buildfix diagnosing and repairing three common Cargo workspace
hygiene issues -- automatically, deterministically, and safely.

## The scenario

We have a small workspace (`repo/`) with three crates:

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
`artifacts/` directory contains two sensor receipts:

- `artifacts/builddiag/report.json` -- reports the missing resolver v2
- `artifacts/depguard/report.json` -- reports duplicate deps and missing versions

From these receipts, buildfix generates a deterministic plan and a unified
patch.

## Running the demo

From the repository root:

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

# Step 3: Apply the fixes
cargo run -p buildfix -- apply \
  --repo-root examples/demo/repo \
  --out-dir examples/demo/output \
  --apply \
  --allow-dirty

# Step 4: Verify the result
cat examples/demo/repo/Cargo.toml
cat examples/demo/repo/crates/api/Cargo.toml
cat examples/demo/repo/crates/cli/Cargo.toml
cat examples/demo/repo/crates/core/Cargo.toml
```

## The plan

Running `buildfix plan` produces 8 operations across 4 files, all classified
as **safe** (fully determined from repo truth, no user input needed):

```
# buildfix plan

- Ops: 8 (blocked 0)
- Files touched: 4
- Patch bytes: 1282
- Safety: 8 safe, 0 guarded, 0 unsafe
- Inputs: 2
```

## The patch

Here is the complete patch that buildfix generates:

```diff
diff --git a/Cargo.toml b/Cargo.toml
--- a/Cargo.toml
+++ b/Cargo.toml
--- original
+++ modified
@@ -1,2 +1,4 @@
 [workspace]
 members = ["crates/api", "crates/core", "crates/cli"]
+resolver = "2"
+dependencies = { serde = "1.0.200" }
diff --git a/crates/api/Cargo.toml b/crates/api/Cargo.toml
--- a/crates/api/Cargo.toml
+++ b/crates/api/Cargo.toml
--- original
+++ modified
@@ -4,5 +4,5 @@
 edition = "2021"

 [dependencies]
-acme-core = { path = "../core" }
-serde = { version = "1.0.180", features = ["derive"] }
+acme-core = { path = "../core" , version = "0.3.0" }
+serde = { workspace = true, features = ["derive"] }
diff --git a/crates/cli/Cargo.toml b/crates/cli/Cargo.toml
--- a/crates/cli/Cargo.toml
+++ b/crates/cli/Cargo.toml
--- original
+++ modified
@@ -4,6 +4,6 @@
 edition = "2021"

 [dependencies]
-acme-core = { path = "../core" }
-acme-api = { path = "../api" }
-serde = "1.0.190"
+acme-core = { path = "../core" , version = "0.3.0" }
+acme-api = { path = "../api" , version = "0.3.0" }
+serde = { workspace = true }
diff --git a/crates/core/Cargo.toml b/crates/core/Cargo.toml
--- a/crates/core/Cargo.toml
+++ b/crates/core/Cargo.toml
--- original
+++ modified
@@ -4,4 +4,4 @@
 edition = "2021"

 [dependencies]
-serde = "1.0.200"
+serde = { workspace = true }
```

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
demo repo is not a real git repository), but in production usage the
precondition system prevents stale plans from being applied.

## What's next

- Run `buildfix list-fixes` to see all available fixers
- Run `buildfix explain <key>` to understand any fixer's safety rationale
- See the [design docs](../../docs/design.md) for architecture details
