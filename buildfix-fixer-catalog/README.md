# buildfix-fixer-catalog

Canonical metadata for enabled built-in fixers.

This crate centralizes:

- user-facing keys (`resolver-v2`, `msrv`, etc.)
- internal fix IDs (`cargo.normalize_edition`, etc.)
- feature-gated trigger patterns used by explain and policy checks

It is a small compatibility layer intended to avoid duplication across core and CLI
surfaces.
