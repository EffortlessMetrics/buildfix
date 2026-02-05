# Roadmap

This document outlines the planned features and improvements for buildfix.

## Current Status

buildfix v0.1 is feature-complete for the core workflow:
- Receipt-driven planning from sensor outputs
- Safe, deterministic TOML editing
- Precondition verification and backup system
- Four built-in fixers covering common Cargo workspace issues

## Planned Features

### Near-Term (v0.2)

#### Edition Upgrade Fixer
- **Fix Key**: `cargo.upgrade_edition`
- **Safety**: Guarded
- **Description**: Normalizes per-crate Rust edition to workspace canonical value
- **Pattern**: Similar to MSRV normalization, reads `[workspace.package].edition`

#### Duplicate Dependency Consolidation Fixer
- **Fix Key**: `cargo.consolidate_duplicate_deps`
- **Safety**: Safe
- **Sensors**: depguard
- **Description**: Identifies dependencies used by multiple crates at different versions and consolidates them to `[workspace.dependencies]`

### Medium-Term (v0.3)

#### Unused Dependency Removal Fixer
- **Fix Key**: `cargo.remove_unused_deps`
- **Safety**: Unsafe (requires user confirmation)
- **Description**: First fixer to use `OpKind::TomlRemove`. Requires sensor providing unused dependency detection.

#### Additional Op Types
- **Anchored text replace**: Support for non-TOML file edits with strict constraints
- **Pattern**: Line-based replacements with context anchors for safety

### Long-Term

#### Auto-Commit Mode
- Optional auto-commit after successful apply (maintainer-only workflow)
- Requires clean working tree and explicit flag
- Commits with structured message referencing plan

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
