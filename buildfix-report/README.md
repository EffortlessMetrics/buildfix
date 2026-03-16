# buildfix-report

`buildfix-report` centralizes report construction for plan and apply outcomes.

## Responsibilities

- Build deterministic `BuildfixReport` summaries from planned operations.
- Build deterministic `ReportCapabilities` from loaded receipts.
- Keep wire-schema payloads aligned between plan/apply command paths.

The crate intentionally contains no filesystem or git side effects; callers provide
typed domain inputs and receive report values.
