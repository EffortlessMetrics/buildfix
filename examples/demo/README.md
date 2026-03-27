# buildfix demo: safe repair of a Cargo workspace

This demo shows the supported lane buildfix is meant to handle today: deterministic, safe repairs driven by sensor receipts. It proves that buildfix can take real receipt data, plan a workspace repair, and apply it without guessing.

## What this demo proves

The sample workspace has three operator-visible problems:

- the root `Cargo.toml` is missing `resolver = "2"`
- the workspace members use inconsistent `serde` versions
- path dependencies are missing publishable `version` fields

buildfix repairs all three with safe operations only. No parameters are required and no human judgment is needed for the edit values.

## Why this lane is supported

The demo uses the two receipt types the repair engine understands in this lane:

- `artifacts/builddiag/report.json` for the workspace resolver finding
- `artifacts/depguard/report.json` for duplicate dependency versions and missing path dependency versions

That is the important proof point: buildfix is not inferring problems from source code. It is consuming receipts and turning them into a deterministic plan.

## Run it

From the `buildfix` repository root:

```bash
cargo run -p buildfix -- plan \
  --repo-root examples/demo/repo \
  --artifacts-dir examples/demo/artifacts \
  --out-dir examples/demo/output \
  --no-clean-hashes

cat examples/demo/output/plan.md
cat examples/demo/output/patch.diff

cargo run -p buildfix -- apply \
  --repo-root examples/demo/repo \
  --out-dir examples/demo/output \
  --apply \
  --allow-dirty
```

After the apply, inspect the repaired manifests:

```bash
cat examples/demo/repo/Cargo.toml
cat examples/demo/repo/crates/api/Cargo.toml
cat examples/demo/repo/crates/cli/Cargo.toml
cat examples/demo/repo/crates/core/Cargo.toml
```

## Why the edits are safe

All edits in this demo are classified as `safe` because buildfix can derive the values from repo truth:

| Fix | Why buildfix can do it without guessing |
|-----|-----------------------------------------|
| `resolver-v2` | Either the workspace sets `resolver = "2"` or it does not |
| `duplicate-deps` | The canonical version comes from the highest reported version in the workspace |
| `path-dep-version` | The version comes from the target crate's own `Cargo.toml` |

The plan is also protected by SHA256 preconditions. If the workspace changes between `plan` and `apply`, buildfix refuses to apply stale output.

## Expected output

The `examples/demo/expected/` directory holds the reference outputs for this demo:

- `plan.json`
- `plan.md`
- `patch.diff`
- `comment.md`
- `report.json`
- `extras/buildfix.report.v1.json`

The structural content is deterministic. Placeholder values such as commit hashes and timestamps vary by machine and run.

## Resetting the demo

Use `git diff examples/demo/repo` to inspect the applied changes. Reset the demo workspace with `git checkout -- examples/demo/repo` when you want to run it again.
