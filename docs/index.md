# buildfix documentation

This site follows [Diataxis](https://diataxis.fr/), but the first stop is the supported operator lane:

- `builddiag` and `depguard` receipts
- safe fixes: `resolver-v2`, `path-dep-version`, `workspace-inheritance`, `duplicate-deps`
- examples: [`examples/demo`](../examples/demo/README.md) and [`examples/profiles`](../examples/profiles/README.md)

Start here if you are using buildfix on a real workspace:

- [Getting Started](tutorials/getting-started.md) - install buildfix and generate your first plan
- [Your First Fix](tutorials/first-fix.md) - walk through one safe plan/apply cycle

Use the rest of the docs by intent:

## Tutorials

Learn the happy path from a receipt to a repaired workspace.

- [Getting Started](tutorials/getting-started.md)
- [Your First Fix](tutorials/first-fix.md)

## How-To Guides

Solve a specific operator problem.

- [Configure buildfix](how-to/configure.md)
- [Integrate with CI/CD](how-to/ci-integration.md)
- [Troubleshoot Blocked Fixes](how-to/troubleshoot.md)
- [Add a New Fixer](how-to/extend-fixers.md)

## Reference

Look up exact commands, schemas, and supported fix behavior.

- [CLI Commands](reference/cli.md)
- [Fix Catalog](reference/fixes.md)
- [Configuration Schema](reference/config.md)
- [Output Schemas](reference/schemas.md)
- [Exit Codes](reference/exit-codes.md)

## Explanation

Read these when you want the rationale, not the procedure.

- [Architecture](architecture.md)
- [Safety Model](safety-model.md)
- [Design Goals](design.md)
- [Requirements](requirements.md)
- [Preconditions](explanation/preconditions.md)
- [Determinism](explanation/determinism.md)
- [Testing Strategy](testing.md)
