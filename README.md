# buildfix

buildfix repairs Cargo workspace hygiene from sensor receipts.

It reads `artifacts/*/report.json`, plans deterministic fixes, and applies them with explicit safety gates.

## What buildfix is for

Use buildfix when you already have sensor output and want a repeatable repair plan for a Cargo workspace.

The currently supported lane is:

- `builddiag` for resolver and workspace-policy findings
- `depguard` for path dependency versions, workspace inheritance, and duplicate dependency versions

Those fixers are safe by default:

- `resolver-v2`
- `path-dep-version`
- `workspace-inheritance`
- `duplicate-deps`

Guarded and unsafe fixes also exist, but they are secondary and require explicit review or parameters.

## Quick start

```bash
cargo install buildfix --locked
buildfix plan
cat artifacts/buildfix/plan.md
buildfix apply
buildfix apply --apply
```

If you want to try it on a known-good example, start with [`examples/demo`](examples/demo/README.md) and [`examples/profiles`](examples/profiles/README.md).

## What you get

`buildfix plan` writes:

- `artifacts/buildfix/plan.json`
- `artifacts/buildfix/plan.md`
- `artifacts/buildfix/patch.diff`
- `artifacts/buildfix/report.json`

`buildfix apply` writes:

- `artifacts/buildfix/apply.json`
- `artifacts/buildfix/apply.md`
- `artifacts/buildfix/patch.diff`
- `artifacts/buildfix/report.json`

## What it will not do

- It does not guess missing values.
- It does not apply unsafe changes without explicit approval.
- It does not repair a workspace with stale receipts without refusing first.

## Read next

- [`docs/index.md`](docs/index.md)
- [`docs/reference/support-matrix.md`](docs/reference/support-matrix.md)
- [`docs/tutorials/getting-started.md`](docs/tutorials/getting-started.md)
- [`docs/tutorials/first-fix.md`](docs/tutorials/first-fix.md)
- [`docs/reference/fixes.md`](docs/reference/fixes.md)
- [`docs/reference/cli.md`](docs/reference/cli.md)
