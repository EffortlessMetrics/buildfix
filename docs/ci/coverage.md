# Coverage

Codecov coverage is Rust execution-surface evidence for the `buildfix` repository.

It answers:
> Did tests execute this Rust surface?

It does not answer:
- whether a fixer is safe,
- whether a generated patch is correct,
- whether receipt adapters are complete,
- whether stale-receipt detection is correct,
- whether safe, guarded, and unsafe fix classification is correct,
- whether conformance coverage is complete,
- whether BDD coverage is adequate,
- whether package or release readiness is proven.

Those are separate proof lanes.

## Workflow

The Coverage workflow runs on:
- push to `main`,
- `workflow_dispatch`,
- PRs labeled `coverage`, `full-ci`, or `ci:full`.

Codecov comments are disabled. Durable receipts are:
- `coverage.json`,
- `coverage.txt`,
- `lcov.info`,
- the GitHub Actions coverage artifact,
- the Codecov dashboard.
