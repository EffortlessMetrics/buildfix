# Changelog

All notable changes to buildfix are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Capabilities block** in receipt and report structures for sensor capability negotiation
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

### Fixed

- Deterministic op ordering using stable sort keys
- TOML formatting preservation during edits
- Proper handling of workspace.dependencies inheritance with features

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
