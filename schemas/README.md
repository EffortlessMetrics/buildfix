# Schemas

These JSON Schemas define buildfix artifacts.

- buildfix.plan.v1.json
- buildfix.apply.v1.json
- buildfix.report.v1.json

Notes:
- `buildfix.report.v1` is designed to be envelope-compatible for cockpit ingestion.
- If your ecosystem vendors a separate `receipt.envelope.v1.json`, you may choose to validate buildfix.report.v1 with `allOf` against that envelope schema instead of treating it as standalone.
