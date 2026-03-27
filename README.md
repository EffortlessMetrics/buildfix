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
cargo install buildfix
buildfix plan
cat artifacts/buildfix/plan.md
buildfix apply
buildfix apply --apply
```

The documented path above is the one we have verified end to end:

- `cargo install buildfix`
- `buildfix --help`
- `buildfix list-fixes`
- `buildfix plan`
- `buildfix apply`
- `buildfix apply --apply`

The current published `0.2.0` release installs cleanly with `cargo install buildfix`.
We treat `cargo install buildfix --locked` as a release-candidate gate until the
packaged lock is refreshed in the next cut.

If you want to try it on a known-good example, start with [`examples/demo`](examples/demo/README.md) and [`examples/profiles`](examples/profiles/README.md). Those examples are the best place to inspect the supported lane before using buildfix on a real workspace.

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
- It does not claim broader support than the safe `builddiag` and `depguard` lane documented here.

## Read next

- [`docs/index.md`](docs/index.md)
- [`docs/reference/support-matrix.md`](docs/reference/support-matrix.md)
- [`docs/tutorials/getting-started.md`](docs/tutorials/getting-started.md)
- [`docs/tutorials/first-fix.md`](docs/tutorials/first-fix.md)
- [`docs/reference/fixes.md`](docs/reference/fixes.md)
- [`docs/reference/cli.md`](docs/reference/cli.md)
