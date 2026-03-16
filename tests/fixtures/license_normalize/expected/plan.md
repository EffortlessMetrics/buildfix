# buildfix plan

- Ops: 1 (blocked 0)
- Files touched: 1
- Safety: 0 safe, 1 guarded, 0 unsafe
- Inputs: 1

## Ops

### 1. 54c07fcf-11fc-5d08-86f2-0da50f659785

- Safety: `guarded`
- Blocked: `false`
- Target: `crates/a/Cargo.toml`
- Kind: `set_package_license`

Normalizes per-crate package.license to workspace canonical license

**Findings**

- `cargo-deny/licenses.unlicensed` `missing_license` at crates/a/Cargo.toml:5

