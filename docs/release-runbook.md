# buildfix Release Runbook

> **Version**: 1.0  
> **Last Updated**: 2026-03-16  
> **Applies to**: v0.3.x releases and beyond

This runbook captures the lessons learned from the v0.2.0 release to make future releases repeatable and require zero institutional knowledge.

Cross-reference: [implementation-plan.md](implementation-plan.md) — v0.2.1 Operational Hardening

---

## 1. Prerequisites

### 1.1 Required Tooling

- **Rust toolchain**: `rustup` with stable toolchain matching `rust-version` in workspace
- **cargo**: Standard Cargo from rustup
- **git**: For tagging and pushing
- **crates.io account**: With owner or owner-like permissions for all `buildfix-*` crates

### 1.2 Environment Setup

```bash
# Verify you're logged into crates.io
cargo login

# Or set the token directly (recommended for CI/scripts)
export CARGO_REGISTRY_TOKEN="your-token-here"

# Verify toolchain matches workspace rust-version
rustup show
rustc --version  # Should be >= 1.92
```

### 1.3 Pre-Release Checks

```bash
# 1. Ensure clean working directory
git status
git diff origin/main

# 2. Run full test suite
cargo test --workspace --exclude buildfix-bdd

# 3. Run BDD tests (optional but recommended)
cargo test -p buildfix-bdd --test cucumber

# 4. Run clippy with all warnings
cargo clippy --workspace --all-targets -- -D warnings

# 5. Verify formatting
cargo fmt --check

# 6. Verify documentation builds
cargo doc --workspace --no-deps

# 7. Verify changelog is updated
# Manually review CHANGELOG.md has entry for new version
```

### 1.4 Version Bump Checklist

Before publishing, update versions in all crate `Cargo.toml` files:

```bash
# Verify all crates have the target version
grep -r "^version = " --include="Cargo.toml" | grep -v target
```

---

## 2. Publish Order

The publish order is determined by the dependency graph. **Do not deviate from this order** or publishes will fail due to missing dependencies.

### Dependency Graph Summary

```
Layer 0: buildfix-types, buildfix-hash (no internal deps)
Layer 1: buildfix-adapter-sdk, buildfix-receipts-sarif, buildfix-render, buildfix-edit
Layer 2: buildfix-fixer-api, buildfix-report, buildfix-artifacts, buildfix-fixer-catalog
Layer 3: buildfix-domain-policy, individual fixer crates, buildfix-core-runtime, intake adapters
Layer 4: buildfix-domain
Layer 5: buildfix-core
Layer 6: buildfix-cli
```

### Exact Publish Sequence

```bash
# === LAYER 0 (no internal dependencies) ===
# These can be published in parallel if desired

cargo publish -p buildfix-types
cargo publish -p buildfix-hash

# === LAYER 1 ===
# Wait for Layer 0 to be indexed by crates.io (30-60 seconds each)

cargo publish -p buildfix-adapter-sdk
cargo publish -p buildfix-receipts-sarif
cargo publish -p buildfix-render
cargo publish -p buildfix-edit

# === LAYER 2 ===
# Wait for Layer 1 to be indexed

cargo publish -p buildfix-fixer-api
cargo publish -p buildfix-report
cargo publish -p buildfix-artifacts
cargo publish -p buildfix-fixer-catalog

# === LAYER 3 ===
# Wait for Layer 2 to be indexed

cargo publish -p buildfix-domain-policy

# Individual fixer crates (can be parallel after fixer-api is indexed)
cargo publish -p buildfix-fixer-resolver-v2
cargo publish -p buildfix-fixer-path-dep-version
cargo publish -p buildfix-fixer-workspace-inheritance
cargo publish -p buildfix-fixer-duplicate-deps
cargo publish -p buildfix-fixer-remove-unused-deps
cargo publish -p buildfix-fixer-msrv
cargo publish -p buildfix-fixer-edition
cargo publish -p buildfix-fixer-license

# Intake adapter crates (depend on adapter-sdk)
cargo publish -p buildfix-receipts-cargo-crev
cargo publish -p buildfix-receipts-cargo-deny
cargo publish -p buildfix-receipts-cargo-udeps
cargo publish -p buildfix-receipts-cargo-machete
cargo publish -p buildfix-receipts-cargo-outdated
cargo publish -p buildfix-receipts-cargo-lock
cargo publish -p buildfix-receipts-cargo-update
cargo publish -p buildfix-receipts-depguard
cargo publish -p buildfix-receipts-rustc-json
cargo publish -p buildfix-receipts-clippy
cargo publish -p buildfix-receipts-rustfmt
cargo publish -p buildfix-receipts-cargo-miri
cargo publish -p buildfix-receipts-cargo-spellcheck
cargo publish -p buildfix-receipts-cargo-audit
cargo publish -p buildfix-receipts-cargo-sec-audit
cargo publish -p buildfix-receipts-cargo-tree
cargo publish -p buildfix-receipts-cargo-bloat
cargo publish -p buildfix-receipts-cargo-llvm-lines
cargo publish -p buildfix-receipts-cargo-cyclonedds
cargo publish -p buildfix-receipts-cargo-geiger
cargo publish -p buildfix-receipts-cargo-semver-checks
cargo publish -p buildfix-receipts-cargo-warn
cargo publish -p buildfix-receipts-cargo-msrv
cargo publish -p buildfix-receipts-cargo-krate
cargo publish -p buildfix-receipts-tarpaulin
cargo publish -p buildfix-receipts-cargo-audit-freeze
cargo publish -p buildfix-receipts-cargo-unused-function

# Core runtime (needs edit + receipts)
cargo publish -p buildfix-core-runtime

# === LAYER 4 ===
# Wait for all Layer 3 crates to be indexed

cargo publish -p buildfix-domain

# === LAYER 5 ===
# Wait for domain to be indexed

cargo publish -p buildfix-core

# === LAYER 6 ===
# Wait for core to be indexed

cargo publish -p buildfix  # This is buildfix-cli, published as "buildfix"
```

### Crates NOT Published

- `buildfix-bdd` — `publish = false` (test-only)
- `xtask` — Not in workspace members for publishing
- `fuzz` — Excluded from workspace

---

## 3. crates.io Pacing Handling

### 3.1 Rate Limiting Behavior

crates.io applies rate limits to prevent abuse. For new crate versions, the index must update before dependents can reference them. Key behaviors:

- **New crate versions**: May take 30-60 seconds to appear in the index
- **Rate limit errors**: HTTP 429 responses with "too many requests" message
- **Index propagation**: `cargo search` may lag behind actual availability

### 3.2 Recommended Wait Times

```bash
# After each publish, wait before publishing dependents
sleep 60  # Recommended for new crates or major version bumps
sleep 30  # Acceptable for patch versions on established crates
```

### 3.3 Detecting Rate-Limit vs Actual Failures

**Rate-limit error (retry after wait):**
```
error: failed to publish to registry at https://crates.io

Caused by:
  the remote server responded with an error (status 429): You have been rate limited.
  Please wait 60 seconds before trying again.
```

**Actual failure (do not retry without fix):**
```
error: failed to publish to registry at https://crates.io

Caused by:
  the remote server responded with an error (status 400): Crate `buildfix-foo` 
  depends on `buildfix-bar ^0.2.0`, but that version does not exist.
```

**Crate already exists (skip, not an error):**
```
error: failed to publish to registry at https://crates.io

Caused by:
  crate version `0.2.0` is already uploaded
```

---

## 4. Retry Behavior

### 4.1 Basic Retry Pattern

```bash
# If a publish fails due to rate limiting, wait and retry
cargo publish -p buildfix-<crate>

# If rate-limited:
sleep 60
cargo publish -p buildfix-<crate>
```

### 4.2 Using --registry Flag

The `--registry` flag is typically not needed for crates.io (the default), but can be useful for testing against alternative registries:

```bash
# Default behavior (crates.io)
cargo publish -p buildfix-<crate>

# Explicit crates.io (equivalent to default)
cargo publish -p buildfix-<crate> --registry crates-io

# Note: Our Cargo.toml files use publish = ["crates-io"], 
# so --registry is only needed if overriding
```

### 4.3 Handling "Crate Already Exists" Errors

This error means the crate version is already published — **this is not a failure**:

```bash
# If you see "crate version `0.2.0` is already uploaded"
# Simply skip that crate and continue to the next one
echo "Crate already published, continuing..."
```

---

## 5. Resume from Crate X Rule

If the release fails mid-way, you don't need to start over. Follow this procedure:

### 5.1 Identify Last Successful Publish

```bash
# Check what's already on crates.io
cargo search buildfix-types
cargo search buildfix-hash
cargo search buildfix-receipts
# ... continue through the list
```

### 5.2 Verify Crate Availability

```bash
# Check if a specific version exists
curl -s https://crates.io/api/v1/crates/buildfix-types/0.2.0 | head -20

# Or use cargo's internal check
cargo search buildfix-types --limit 1
```

### 5.3 Resume Command Pattern

Start from the first crate that failed. All earlier crates in the sequence can be skipped:

```bash
# Example: If buildfix-domain failed, verify its dependencies are published
cargo search buildfix-domain-policy
cargo search buildfix-fixer-catalog
cargo search buildfix-fixer-api

# All dependencies present? Resume from buildfix-domain
cargo publish -p buildfix-domain

# Continue with remaining crates in order
sleep 30
cargo publish -p buildfix-core
sleep 30
cargo publish -p buildfix
```

### 5.4 Quick Resume Script

```bash
#!/bin/bash
# resume-publish.sh - Resume from a specific crate

CRATES=(
  "buildfix-types"
  "buildfix-hash"
  "buildfix-adapter-sdk"
  "buildfix-receipts-sarif"
  "buildfix-render"
  "buildfix-edit"
  "buildfix-fixer-api"
  "buildfix-report"
  "buildfix-artifacts"
  "buildfix-fixer-catalog"
  "buildfix-domain-policy"
  "buildfix-fixer-resolver-v2"
  "buildfix-fixer-path-dep-version"
  "buildfix-fixer-workspace-inheritance"
  "buildfix-fixer-duplicate-deps"
  "buildfix-fixer-remove-unused-deps"
  "buildfix-fixer-msrv"
  "buildfix-fixer-edition"
  "buildfix-fixer-license"
  "buildfix-receipts-cargo-crev"
  "buildfix-receipts-cargo-deny"
  "buildfix-receipts-cargo-udeps"
  "buildfix-receipts-cargo-machete"
  "buildfix-receipts-cargo-outdated"
  "buildfix-receipts-cargo-lock"
  "buildfix-receipts-cargo-update"
  "buildfix-receipts-depguard"
  "buildfix-receipts-rustc-json"
  "buildfix-receipts-clippy"
  "buildfix-receipts-rustfmt"
  "buildfix-receipts-cargo-miri"
  "buildfix-receipts-cargo-spellcheck"
  "buildfix-receipts-cargo-audit"
  "buildfix-receipts-cargo-sec-audit"
  "buildfix-receipts-cargo-tree"
  "buildfix-receipts-cargo-bloat"
  "buildfix-receipts-cargo-llvm-lines"
  "buildfix-receipts-cargo-cyclonedds"
  "buildfix-receipts-cargo-geiger"
  "buildfix-receipts-cargo-semver-checks"
  "buildfix-receipts-cargo-warn"
  "buildfix-receipts-cargo-msrv"
  "buildfix-receipts-cargo-krate"
  "buildfix-receipts-tarpaulin"
  "buildfix-receipts-cargo-audit-freeze"
  "buildfix-receipts-cargo-unused-function"
  "buildfix-core-runtime"
  "buildfix-domain"
  "buildfix-core"
  "buildfix"
)

START_FROM="${1:-buildfix-types}"
FOUND=0

for crate in "${CRATES[@]}"; do
  if [ "$FOUND" -eq 0 ] && [ "$crate" = "$START_FROM" ]; then
    FOUND=1
  fi
  
  if [ "$FOUND" -eq 1 ]; then
    echo "Publishing $crate..."
    cargo publish -p "$crate" || echo "Failed or already exists: $crate"
    sleep 30
  fi
done
```

Usage:
```bash
chmod +x resume-publish.sh
./resume-publish.sh buildfix-domain  # Resume from buildfix-domain
```

---

## 6. Post-Release Verification

### 6.1 Verify All Crates Are Published

```bash
# Check all published crates
for crate in buildfix-types buildfix-hash buildfix-adapter-sdk \
             buildfix-receipts-sarif buildfix-render buildfix-edit \
             buildfix-fixer-api buildfix-report buildfix-artifacts \
             buildfix-fixer-catalog buildfix-domain-policy buildfix-fixer-resolver-v2 \
             buildfix-fixer-path-dep-version buildfix-fixer-workspace-inheritance \
             buildfix-fixer-duplicate-deps buildfix-fixer-remove-unused-deps \
             buildfix-fixer-msrv buildfix-fixer-edition buildfix-fixer-license \
             buildfix-receipts-cargo-deny buildfix-receipts-cargo-machete \
             buildfix-receipts-depguard buildfix-receipts-clippy \
             buildfix-core-runtime buildfix-domain buildfix-core buildfix; do
  echo -n "$crate: "
  cargo search "$crate" 2>/dev/null | grep "^$crate = " || echo "NOT FOUND"
done
```

### 6.2 Test Installation

```bash
# In a fresh directory (not the workspace)
cd /tmp
mkdir buildfix-test && cd buildfix-test
cargo init

# Add buildfix as dependency
echo '[dependencies]
buildfix = "0.2"' >> Cargo.toml

# Verify it resolves
cargo fetch

# Or install the CLI globally
cargo install buildfix --version 0.2.0

# Verify the CLI works
buildfix --version
buildfix --help
```

### 6.3 Create Git Tag

```bash
# Tag the release
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```

### 6.4 Create GitHub Release

1. Go to https://github.com/EffortlessMetrics/buildfix/releases/new
2. Select the tag (e.g., `v0.2.0`)
3. Title: `v0.2.0`
4. Copy release notes from `CHANGELOG.md`
5. Attach any relevant artifacts (optional)
6. Publish release

### 6.5 Update Documentation

```bash
# Verify docs.rs built successfully
open https://docs.rs/buildfix/0.2.0/buildfix/

# Check crate page on crates.io
open https://crates.io/crates/buildfix
```

---

## 7. Troubleshooting

### 7.1 Common Error Messages

| Error | Cause | Solution |
|-------|-------|----------|
| `crate version X.Y.Z is already uploaded` | Version already on crates.io | Skip and continue (not an error) |
| `depends on CRATE ^X.Y.Z, but that version does not exist` | Dependency not yet indexed | Wait 30-60 seconds and retry |
| `status 429: You have been rate limited` | Too many requests | Wait specified time and retry |
| `failed to verify the checksum` | Network corruption or tampering | Retry; if persists, investigate |
| `crate name is too similar to existing crate` | Naming conflict | Rename crate (rare, requires yank) |
| `the crate size is too large` | Files exceed 10MB limit | Exclude large files with `exclude` in Cargo.toml |

### 7.2 When to Yank a Release

Yank a release **only** if:
- Critical security vulnerability discovered
- Breaking change accidentally introduced in a patch release
- Crate is fundamentally broken (doesn't compile, runtime crash)

**Do NOT yank for:**
- Minor bugs (release a patch instead)
- Documentation issues
- Feature requests

### 7.3 How to Yank

```bash
# Yank a specific version
cargo yank --vers 0.2.0 buildfix-types

# This prevents new projects from using that version
# Existing Cargo.lock files continue to work
```

### 7.4 Contacting crates.io Support

- **GitHub Issues**: https://github.com/rust-lang/crates.io/issues
- **Email**: help@crates.io
- **Documentation**: https://doc.rust-lang.org/cargo/reference/registries.html

---

## 8. Quick Reference Card

```bash
# === PRE-RELEASE ===
cargo test --workspace --exclude buildfix-bdd
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check

# === PUBLISH (in order; use resume-publish.sh for full sequence) ===
# See Section 2 "Exact Publish Sequence" for the complete list.
# Quick summary of the dependency layers:
cargo publish -p buildfix-types && sleep 30
cargo publish -p buildfix-hash && sleep 30
# ... Layer 1-3 adapters & fixers (see Section 2) ...
cargo publish -p buildfix-core-runtime && sleep 30
cargo publish -p buildfix-domain && sleep 30
cargo publish -p buildfix-core && sleep 30
cargo publish -p buildfix

# === POST-RELEASE ===
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
cargo install buildfix --version 0.2.0
buildfix --version
```

---

## Changelog

| Date | Version | Changes |
|------|---------|---------|
| 2026-03-20 | 1.1 | Updated dependency graph to include all intake adapters; added missing adapter crates to publish sequence |
| 2026-03-16 | 1.0 | Initial runbook created from v0.2.0 release lessons |
