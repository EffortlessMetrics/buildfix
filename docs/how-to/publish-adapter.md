# How to Publish an Adapter

This guide walks you through publishing your buildfix adapter to crates.io and making it discoverable by the community.

## 1. Prerequisites

Before publishing, ensure you have:

- **crates.io account**: Sign up at [crates.io](https://crates.io) using your GitHub account
- **API token configured**: Run `cargo login` and paste your API token from [crates.io/me](https://crates.io/me)
- **Fully tested adapter**: All tests pass and the adapter works with real sensor output
- **Documented code**: Both inline documentation and README files are complete

> **Note**: If you haven't written an adapter yet, start with [How to Write an Adapter](./write-adapter.md).

## 2. Pre-Publish Checklist

Run through this checklist before publishing:

- [ ] All tests pass: `cargo test -p buildfix-receipts-your-sensor`
- [ ] Clippy clean: `cargo clippy -p buildfix-receipts-your-sensor -- -D warnings`
- [ ] Documentation complete:
  - [ ] `CLAUDE.md` with sensor-specific guidance
  - [ ] `README.md` with usage examples
  - [ ] Inline rustdoc comments
- [ ] `Cargo.toml` metadata complete (see [section 3](#3-cargotoml-requirements))
- [ ] `CHANGELOG.md` updated with this version's changes
- [ ] Version number follows [semantic versioning](#51-semantic-versioning-basics)
- [ ] Dry run succeeds: `cargo publish --dry-run`

## 3. Cargo.toml Requirements

Your `Cargo.toml` must include complete metadata for crates.io discoverability:

```toml
[package]
name = "buildfix-receipts-your-sensor"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"              # Minimum supported Rust version
license = "MIT OR Apache-2.0"      # Dual licensing like buildfix
description = "buildfix adapter for your-sensor static analysis output"
repository = "https://github.com/your-org/buildfix"
homepage = "https://github.com/your-org/buildfix#readme"
readme = "README.md"
keywords = ["buildfix", "adapter", "your-sensor", "static-analysis", "lint"]
categories = ["development-tools", "development-tools::build-utils"]
publish = ["crates-io"]            # Explicit publish registry

[dependencies]
buildfix-adapter-sdk = "0.3"       # Use published version after SDK is released
buildfix-types = "0.3"
anyhow = "1.0"
camino = "1.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"

[dev-dependencies]
pretty_assertions = "1.4"
tempfile = "3.10"
```

### Required Fields Explained

| Field | Purpose |
|-------|---------|
| `name` | Must start with `buildfix-receipts-` for consistency |
| `version` | Semantic version; start at `0.1.0` for new adapters |
| `license` | Use `MIT OR Apache-2.0` to match the buildfix ecosystem |
| `description` | Clear, one-line summary for crates.io search |
| `repository` | Full URL to the source repository |
| `keywords` | Include `buildfix`, `adapter`, and sensor-specific terms |
| `categories` | Use `development-tools` as the primary category |

## 4. Publishing Steps

### 4.1 Prepare the Package

Ensure your adapter builds cleanly in release mode:

```bash
cargo build -p buildfix-receipts-your-sensor --release
```

### 4.2 Dry Run

Test the publishing process without actually uploading:

```bash
cargo publish -p buildfix-receipts-your-sensor --dry-run
```

This validates:
- Package structure is correct
- All required metadata is present
- Dependencies are resolvable
- No crate name conflicts exist

### 4.3 Publish to crates.io

Once the dry run succeeds, publish for real:

```bash
cargo publish -p buildfix-receipts-your-sensor
```

You'll see output like:

```
Updating crates.io index
Verifying buildfix-receipts-your-sensor v0.1.0
Compiling buildfix-receipts-your-sensor v0.1.0
Uploading buildfix-receipts-your-sensor v0.1.0
```

### 4.4 Verify Publication

After publishing, verify your crate appears on crates.io:

```bash
# Check crate info
cargo search buildfix-receipts-your-sensor

# View on web
# https://crates.io/crates/buildfix-receipts-your-sensor
```

## 5. Version Management

### 5.1 Semantic Versioning Basics

Follow [Semantic Versioning 2.0](https://semver.org/):

| Bump Type | When to Use | Example |
|-----------|-------------|---------|
| **Major** | Breaking API changes | `0.1.0` → `1.0.0` |
| **Minor** | New features, backward compatible | `0.1.0` → `0.2.0` |
| **Patch** | Bug fixes only | `0.1.0` → `0.1.1` |

### 5.2 Pre-1.0 Versioning

Before `1.0.0`, the API is considered unstable. Use this period to:

- Iterate on the adapter interface
- Gather feedback from early adopters
- Stabilize the receipt schema mapping

**Recommendation**: Start at `0.1.0` and bump minor for any significant changes.

### 5.3 Breaking Changes in Adapters

These changes require a major version bump:

- Changing the adapter trait implementation signature
- Removing support for previously supported sensor output formats
- Renaming public types or functions
- Changing the `ReceiptEnvelope` structure in incompatible ways

These changes are **not** breaking (minor bump):

- Adding new optional fields to parsed output
- Adding new public functions
- Improving error messages
- Adding support for new sensor output versions

## 6. Post-Publish

### 6.1 Update Documentation

After successful publication:

1. **Update project README**: Add your adapter to the list of available adapters
2. **Update CLAUDE.md**: Add any lessons learned or sensor-specific notes
3. **Create a release tag**: `git tag v0.1.0 && git push origin v0.1.0`

### 6.2 Announce Your Adapter

Share your adapter with the community:

- **GitHub Discussions**: Post in the buildfix discussions forum
- **Release Notes**: Include adapter updates in the main project CHANGELOG
- **Social**: Share on relevant Rust communities (Reddit r/rust, Discord, etc.)

### 6.3 Maintenance Expectations

Published adapters should:

- Respond to issues within a reasonable timeframe
- Update dependencies for security vulnerabilities
- Test against new versions of the sensor tool
- Follow buildfix API updates

## 7. CI Integration

### 7.1 Automated Publishing Workflow

Create `.github/workflows/publish-adapter.yml` for automated publishing:

```yaml
name: Publish Adapter

on:
  push:
    tags:
      - 'buildfix-receipts-*'

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Publish to crates.io
        env:
          CARGO_REGISTRY_TOKEN: ${{ secrets.CRATES_IO_TOKEN }}
        run: |
          # Extract package name from tag
          PACKAGE=${GITHUB_REF#refs/tags/}
          PACKAGE=${PACKAGE%-*}  # Remove version
          cargo publish -p $PACKAGE

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          generate_release_notes: true
```

### 7.2 Pre-Publish CI Checks

Add these checks to your PR workflow:

```yaml
name: Adapter CI

on:
  pull_request:
    paths:
      - 'buildfix-receipts-*/**'

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - name: Check formatting
        run: cargo fmt -p buildfix-receipts-your-sensor --check

      - name: Clippy
        run: cargo clippy -p buildfix-receipts-your-sensor -- -D warnings

      - name: Test
        run: cargo test -p buildfix-receipts-your-sensor

      - name: Documentation
        run: cargo doc -p buildfix-receipts-your-sensor --no-deps
```

For more CI patterns, see [CI Integration Guide](./ci-integration.md).

## 8. Troubleshooting

### 8.1 Common Publishing Errors

#### Crate Name Already Exists

```
error: crate `buildfix-receipts-mytool` already exists
```

**Solution**: Choose a different name or contact the existing owner if you believe there's a conflict.

#### Version Already Published

```
error: crate version `0.1.0` is already uploaded
```

**Solution**: Bump the version in `Cargo.toml`. You cannot republish the same version.

#### Missing Required Fields

```
error: missing required field `description` in Cargo.toml
```

**Solution**: Add all required metadata fields (see [section 3](#3-cargotoml-requirements)).

#### Dependency Not Found

```
error: no matching package named `buildfix-adapter-sdk` found
```

**Solution**: Ensure dependencies are published to crates.io, or use path dependencies for workspace-only development.

### 8.2 crates.io Rate Limits

crates.io has rate limits to prevent abuse:

- **Anonymous downloads**: 60 requests per minute
- **Authenticated**: Higher limits with API token
- **Publishing**: 1 crate per 5 minutes (anti-spam)

If you hit rate limits during CI, cache dependencies or use authenticated requests.

### 8.3 Dependency Version Conflicts

If users report dependency conflicts:

1. **Widen version bounds** in your `Cargo.toml`:
   ```toml
   # Instead of exact versions
   serde = "1.0.190"
   
   # Use semantic version ranges
   serde = "1.0"
   ```

2. **Use workspace inheritance** when developing within the buildfix monorepo:
   ```toml
   [dependencies]
   serde.workspace = true
   ```

3. **Check for duplicate versions**:
   ```bash
   cargo tree -d
   ```

### 8.4 Yanking a Bad Release

If you publish a broken version, yank it to prevent new downloads:

```bash
cargo yank --vers 0.1.2 buildfix-receipts-your-sensor
```

**Note**: Yanked versions remain downloadable for existing `Cargo.lock` files but won't be selected for new builds.

## See Also

- [How to Write an Adapter](./write-adapter.md) - Development guide
- [CI Integration Guide](./ci-integration.md) - Full CI/CD patterns
- [Receipt Schema Reference](../reference/receipt-schema.md) - Receipt format documentation
- [crates.io Publishing Guide](https://doc.rust-lang.org/cargo/reference/publishing.html) - Official Cargo documentation
