# Fixtures

This folder is a placeholder for buildfix test fixtures.

Recommended structure:

tests/fixtures/<case>/
  repo/           # a small repository snapshot (files only, no .git required)
  receipts/       # input receipts (artifacts/**/report.json)
  expected/
    plan.json
    plan.md
    patch.diff
    apply.json     # optional
    report.json    # optional

Fixtures should be minimal and explicit: they exist to make outputs deterministic and reviewable.
