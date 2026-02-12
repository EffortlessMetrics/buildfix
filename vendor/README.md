# Vendor Directory

This directory contains vendored copies of external contracts and schemas
used by buildfix at build time.

## Contents

| Directory | Source | Purpose |
|-----------|--------|---------|
| `cockpit-contracts/` | Cockpit ecosystem | Shared sensor/report schemas |

## Updating

To update a vendored dependency:

1. Copy the new version into the appropriate subdirectory.
2. Run `cargo build` to verify `include_str!` paths still resolve.
3. Run `cargo xtask conform --artifacts-dir artifacts/buildfix` to validate.
4. Commit the update with a clear provenance note in the commit message.

## Provenance

Each vendored subdirectory includes its own README with version
and origin information.
