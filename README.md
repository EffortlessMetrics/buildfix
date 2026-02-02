# buildfix — architecture kit

This folder is a **copy-ready** documentation + contract bundle for `buildfix`, the **actuator** in a receipts-first PR cockpit ecosystem.

It includes:

- Requirements, design, architecture, and implementation plan (markdown)
- Fix registry and safety model (markdown)
- Test strategy (BDD + fixtures + snapshots + proptest + fuzz + mutation)
- JSON Schemas for:
  - `buildfix.plan.v1`
  - `buildfix.apply.v1`
  - `buildfix.report.v1` (envelope-compatible summary for cockpit display)
- Example configs (`buildfix.toml`, `cockpit.toml` snippet)
- Gherkin feature files for BDD

## Intended use

- Drop the docs into your `buildfix` repo (`docs/` + `schemas/`).
- Use the schemas to validate artifacts in CI.
- Use the feature files as the starting point for BDD step definitions.

## Canonical buildfix artifacts

```
artifacts/buildfix/
  plan.json      # buildfix.plan.v1
  plan.md        # human summary
  patch.diff     # unified diff preview
  apply.json     # buildfix.apply.v1
  report.json    # buildfix.report.v1 (optional; for cockpit ingestion)
```

## Philosophy (non-negotiables)

- **No writes without explicit apply.**
- **Plan → patch → apply**, always auditably reversible.
- **No builds/tests/benchmarks.** Proof stays in sensors + CI.
- **No “inventing” versions.** If it’s ambiguous, it’s unsafe.
- **Receipts are the interface**: buildfix depends on envelope semantics, not sensor internals.
