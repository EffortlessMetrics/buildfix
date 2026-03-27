# Changelog

All notable changes to buildfix are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1] - 2026-03-27

### Added

- **Problem-first operator docs**: Rewrote the top-level README, tutorials, and support matrix around the safe `builddiag` and `depguard` lane instead of the internal crate layout.
- **Operator proof artifacts**: Added a dogfood demo plus checked-in example profiles that show raw receipts, plan output, patch preview, and apply results for the supported lane.
- **Supported-lane integration coverage**: Added end-to-end tests that keep receipt intake, normalization, planning, patch generation, and apply behavior green for the first supported sensor paths.
- **Install and release smoke coverage**: Added clean cargo-home install checks, supported-lane CLI smoke tests, and refusal-case assertions for policy blocks and stale-plan exits.
- **Unused Dependency Removal Fixer** (`cargo.remove_unused_deps`): Creates deterministic `toml_remove` plan ops from sensor-reported unused dependency paths. Classified as Unsafe and requires `buildfix apply --apply --allow-unsafe`.
- **Golden fixture + BDD coverage** for unused dependency removal, including safety-gate behavior (blocked without `--allow-unsafe`, applied with `--allow-unsafe`).

### Changed

- **Release alignment**: Coordinated the publishable workspace crates onto `0.3.1` so the CLI and its internal dependency closure can be tagged and published together.
- **Release runbook**: Split the locked-install verification into a pre-tag source gate and a post-publish crates.io confirmation so release docs match what can be proven at each stage.
- **Public install guidance**: Kept the public README and tutorial path on `cargo install buildfix` until the next published cut is verified from crates.io with `--locked`.

### Fixed

- **Locked-install release blocker**: Refreshed the source lock so the release-candidate `--locked` install path resolves `rustls-webpki 0.103.10` and passes `cargo audit` with the documented ignore set.
- **Publishability drift**: Removed the mismatch where current source depended on unreleased internal crate versions that were not available from crates.io.

## [0.2.0] - 2026-02-16

### Added

- **Edition Upgrade Fixer** (`cargo.normalize_edition`): Normalizes per-crate Rust edition to workspace canonical value. Classified as Guarded; falls back to Unsafe when no workspace edition is defined (requires `--param edition=<value>`)
- **Capabilities block** in receipt and report structures for "No Green By Omission" sensor capability negotiation
- **Wire representation** (`buildfix-types/wire`) with versioned V1 formats for plan, apply, and report artifacts
- **JSON schemas** for `buildfix.plan.v1.json`, `buildfix.apply.v1.json`, and `buildfix.report.v1.json`
- **Error handling scenarios** in BDD tests for corrupted/incomplete receipts
- **Plan application logic** improvements with better error recovery
- **BDD scenario expansion** including:
  - Max files cap enforcement (`--max-files`)
  - Max patch bytes cap enforcement (`--max-patch-bytes`)
  - Denylist policy enforcement
  - Allowlist policy enforcement
  - Unsafe fix parameter handling
  - Dirty working tree detection
  - Precondition mismatch handling
  - Idempotency verification scenarios
  - Dev-dependency workspace inheritance
  - Feature preservation in workspace inheritance
  - Artifact validation scenarios
- **CLI commands**:
  - `buildfix explain <fix>` - detailed fix explanations
  - `buildfix list-fixes` - list available fixes with JSON output support
  - `buildfix validate` - validate receipts and artifacts against schemas

### Changed

- Improved plan policy filtering with glob pattern matching
- Enhanced precondition verification with dirty working tree checks
- Better error messages for blocked operations
- Cargo.toml dependencies updated to use workspace inheritance

### Fixed

- Deterministic op ordering using stable sort keys
- TOML formatting preservation during edits
- Proper handling of workspace.dependencies inheritance with features
- Clippy and rustfmt compliance across all crates

## [0.1.0] - Initial Release

### Added

- Core planning engine with receipt-driven architecture
- TOML editing engine with format-preserving transformations
- Four built-in fixers:
  - `resolver-v2`: Sets workspace resolver to "2"
  - `path-dep-version`: Adds version to path dependencies
  - `workspace-inheritance`: Converts deps to workspace = true
  - `msrv`: Normalizes per-crate MSRV to workspace value
- Safety model with Safe/Guarded/Unsafe classifications
- Precondition system with SHA256 file hashes
- Backup system before applying changes
- Policy system with allow/deny lists and caps (max_ops, max_files, max_patch_bytes)
- JSON and Markdown artifact outputs
- Unified diff patch generation
