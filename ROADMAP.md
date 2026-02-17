# Roadmap

This document outlines the planned features and improvements for buildfix.

## Current Status

buildfix is feature-complete with seven built-in fixers:
- Receipt-driven planning from sensor outputs
- Safe, deterministic TOML editing
- Precondition verification and backup system
- Six built-in fixers covering common Cargo workspace issues
- Wire format with versioned JSON schemas (V1)
- Capabilities block for sensor capability negotiation

## Completed

### v0.2

- **Edition Upgrade Fixer** (`cargo.normalize_edition`): Normalizes per-crate Rust edition to workspace canonical value. Guarded safety; falls back to Unsafe when no canonical edition exists.
- **Wire representation**: Versioned V1 formats for plan, apply, and report artifacts
- **JSON schemas**: Embedded schemas for artifact validation
- **Capabilities block**: "No Green By Omission" pattern for tracking input availability
- **CLI commands**: `explain`, `list-fixes`, `validate`

### v0.3

- **Duplicate Dependency Consolidation Fixer** (`cargo.consolidate_duplicate_deps`): Consolidates duplicate member dependency versions into `[workspace.dependencies]` and rewrites members to `workspace = true`. Safe safety class; receipt-driven from depguard findings.

### v0.4

- **Unused Dependency Removal Fixer** (`cargo.remove_unused_deps`): Removes dependency entries reported as unused by sensors using deterministic `TomlRemove` ops. Unsafe safety class; apply requires `--allow-unsafe`.
- **Anchored Text Replace Op** (`text_replace_anchored`): Line-based non-TOML edits with strict before/after anchors and bounded replacement count.

### v0.5

- **Auto-Commit Mode**: Optional `buildfix apply --apply --auto-commit` flow with clean-tree enforcement and structured commit metadata in apply artifacts.

## Planned Features

### Medium-Term (v0.4)

#### Additional Op Types
- **Anchored text replace**: Support for non-TOML file edits with strict constraints
- **Pattern**: Line-based replacements with context anchors for safety

### Long-Term (v0.5+)

#### Additional File Format Support
- Support for file types beyond TOML when edits are provably mechanical
- Examples: JSON, YAML configuration files
- Requires format-preserving parsers

#### Sensor Integrations
- First-party integrations with common Rust ecosystem tools:
  - cargo-udeps (unused dependencies)
  - cargo-deny (license compliance)
  - cargo-machete (unused dependencies)

## Design Principles

These principles guide all roadmap items:

1. **Receipt-driven**: All fixes triggered by sensor findings, never invented
2. **Deterministic**: Same inputs always produce byte-identical outputs
3. **Safety-first**: Conservative classification, explicit approval for risky changes
4. **Reversible**: Backups and preconditions ensure recovery
5. **Transparent**: Full audit trail in JSON artifacts

## Contributing

Feature requests and sensor integration ideas are welcome. Please open an issue to discuss before implementing.

When proposing new fixers, include:
- Receipt format from the triggering sensor
- Safety classification rationale
- Example input/output transformation
