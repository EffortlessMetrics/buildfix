# CLAUDE.md

Core report projection for buildfix plan/apply outcomes.

## Build & Test

```bash
cargo build -p buildfix-report
cargo test -p buildfix-report
```

## Description

Generates `BuildfixReport` from plan/apply outcomes. Aggregates receipt capabilities and produces machine-readable reports with Pass/Warn/Fail status.

## Key Functions

- `build_report_capabilities()` — aggregate available check_ids, scopes from receipts
- `build_plan_report()` — create report from a plan
- `build_apply_report()` — create report from an apply

## Report Status

- **Pass**: no ops, no failed inputs (plan); all applied (apply)
- **Warn**: has ops or failed inputs (plan); has blocked (apply)
- **Fail**: never (plan); has failed operations (apply)

## Key Types

- `ReportCapabilities` — available inputs, failed inputs, check_ids, scopes
- `ReportFinding` — individual finding (e.g., receipt_load_failed)
- `ReportVerdict` — status + counts (info/warn/error) + reasons

## Special Considerations

- Input capabilities are sorted and deduplicated
- Failed receipt loads are surfaced as findings with code `receipt_load_failed`
- Plan reports include safety counts and top blocked reason tokens
- Apply reports include auto-commit info when enabled
- Git head SHA tracked through plan/apply lifecycle
