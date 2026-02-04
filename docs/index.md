# buildfix Documentation

**buildfix** is a receipt-driven repair tool for Cargo workspace hygiene. It consumes sensor receipts and emits deterministic repair plans.

This documentation follows the [Diataxis](https://diataxis.fr/) framework, organized into four categories:

## Tutorials

Step-by-step guides for learning buildfix.

- [Getting Started](tutorials/getting-started.md) — Install and run your first plan
- [Your First Fix](tutorials/first-fix.md) — Walk through the plan/apply workflow

## How-To Guides

Task-oriented recipes for common problems.

- [Configure buildfix](how-to/configure.md) — Set up buildfix.toml policy
- [Integrate with CI/CD](how-to/ci-integration.md) — Run buildfix in automated pipelines
- [Troubleshoot Blocked Fixes](how-to/troubleshoot.md) — Debug why fixes aren't applying
- [Add a New Fixer](how-to/extend-fixers.md) — Extend buildfix with custom fixes

## Reference

Precise technical specifications.

- [CLI Commands](reference/cli.md) — Complete command and option reference
- [Fix Catalog](reference/fixes.md) — All available fixes with triggers and safety classes
- [Configuration Schema](reference/config.md) — buildfix.toml specification
- [Output Schemas](reference/schemas.md) — plan.json, apply.json, report.json formats
- [Exit Codes](reference/exit-codes.md) — Exit code semantics

## Explanation

Background and design rationale.

- [Architecture](architecture.md) — Crate responsibilities and data flow
- [Safety Model](safety-model.md) — Safe/guarded/unsafe classification
- [Design Goals](design.md) — Design principles and key objects
- [Requirements](requirements.md) — Scope, inputs, outputs, invariants
- [Preconditions](explanation/preconditions.md) — How drift detection works
- [Determinism](explanation/determinism.md) — Why byte-stable outputs matter
- [Testing Strategy](testing.md) — BDD, golden fixtures, property tests, fuzzing
