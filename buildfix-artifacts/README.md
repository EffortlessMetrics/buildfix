# buildfix-artifacts

Artifact persistence helpers for `buildfix-core`.

- Serialize `BuildfixPlan`/`BuildfixApply`/`BuildfixReport` into canonical JSON and
  markdown artifacts.
- Emit companion schema-marked `buildfix.report.v1.json` in `extras/`.
- Provide a small writer trait for dependency injection and filesystem adapter.
