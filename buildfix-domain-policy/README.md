# buildfix-domain-policy

Reusable policy primitives and deterministic planning helpers used by buildfix domain layer.

This crate centralizes:

- Policy matching helpers (`allow`/`deny` filtering and glob matching)
- Parameter hydration (`params_required` filling)
- Capacity caps (`max_ops`/`max_files`)
- Stable op ordering and deterministic operation IDs
- Stable JSON argument fingerprints

Its goal is to keep these behaviors as a reusable, independently testable contract
between planners and future orchestration components.
