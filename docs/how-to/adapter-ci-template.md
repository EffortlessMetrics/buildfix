# Adapter CI Template

This guide explains how to set up continuous integration for your buildfix adapter using the provided CI workflow template.

## Quick Start

1. Copy the workflow file to your adapter repository:

   ```bash
   mkdir -p .github/workflows
   cp .github/workflows/adapter-ci.yml .github/workflows/ci.yml
   ```

2. Commit and push to your repository:

   ```bash
   git add .github/workflows/ci.yml
   git commit -m "Add CI workflow for adapter"
   git push
   ```

3. The workflow will automatically run on push to `main`/`master` and on pull requests.

## Status Badge

Add a status badge to your README.md to show the CI status:

```markdown
[![Adapter CI](https://github.com/YOUR-ORG/YOUR-REPO/actions/workflows/ci.yml/badge.svg)](https://github.com/YOUR-ORG/YOUR-REPO/actions/workflows/ci.yml)
```

Replace `YOUR-ORG/YOUR-REPO` with your actual GitHub organization and repository name.

## Workflow Features

### Jobs Overview

| Job | Description | Platforms |
|-----|-------------|-----------|
| `test` | Run tests and clippy | Ubuntu, Windows, macOS |
| `validate` | Format check, documentation | Ubuntu |
| `msrv` | Minimum Supported Rust Version | Ubuntu |
| `publish-dry-run` | Test crate publishing | Ubuntu |
| `adapter-harness` | Validate adapter fixtures | Ubuntu |
| `security` | Security audit with cargo-audit | Ubuntu |

### Triggers

The workflow runs on:

- **Push** to `main` or `master` branches
- **Pull requests** targeting `main` or `master`
- **Manual trigger** via `workflow_dispatch`

### Caching

The workflow uses [`Swatinem/rust-cache@v2`](https://github.com/Swatinem/rust-cache) for caching Rust dependencies. This significantly speeds up builds by caching:

- Compiled dependencies
- Build artifacts
- Cargo index

## Customization

### Adjusting the MSRV

The MSRV job reads the `rust-version` field from your `Cargo.toml`. Ensure it's set:

```toml
[package]
name = "buildfix-receipts-your-sensor"
version = "0.1.0"
rust-version = "1.75"
```

To disable MSRV checking, remove the `msrv` job from the workflow or set `continue-on-error: true`.

### Adding Platform-Specific Tests

If your adapter has platform-specific behavior, extend the test matrix:

```yaml
test:
  strategy:
    matrix:
      include:
        - os: ubuntu-latest
          features: "linux-features"
        - os: windows-latest
          features: "windows-features"
        - os: macos-latest
          features: "macos-features"
```

### Custom Test Commands

Modify the test step to use custom commands:

```yaml
- name: Run tests
  run: cargo test --all-features -- --test-threads=1
```

### Adding Security Exceptions

If you need to ignore specific security advisories, update the security job:

```yaml
- name: Install cargo-audit
  uses: rustsec/audit-action@v2
  with:
    ignore: |
      RUSTSEC-2024-0384
      RUSTSEC-2023-0071
```

### Conditional Publishing

To enable actual publishing (not just dry-run), add a separate workflow or modify the `publish-dry-run` job:

```yaml
publish:
  needs: [test, validate]
  runs-on: ubuntu-latest
  if: startsWith(github.ref, 'refs/tags/')
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - run: cargo publish --token ${{ secrets.CRATES_IO_TOKEN }}
```

> **Warning**: Store your crates.io API token as a GitHub secret, never commit it to the repository.

## Integration with Existing CI

### Merging with Existing Workflows

If you already have CI workflows, you can:

1. **Copy individual jobs** - Extract specific jobs from the template
2. **Reuse steps** - Copy individual steps into your existing jobs
3. **Use as reference** - Follow the patterns without copying directly

### Required Permissions

The workflow uses the default `GITHUB_TOKEN`. No additional permissions or secrets are required for basic operation.

For publishing, you'll need:

| Secret | Purpose |
|--------|---------|
| `CRATES_IO_TOKEN` | API token for crates.io publishing |

### Workflow Dependencies

The workflow uses these well-maintained actions:

- [`actions/checkout@v4`](https://github.com/actions/checkout) - Repository checkout
- [`dtolnay/rust-toolchain@stable`](https://github.com/dtolnay/rust-toolchain) - Rust installation
- [`Swatinem/rust-cache@v2`](https://github.com/Swatinem/rust-cache) - Dependency caching
- [`rustsec/audit-action@v2`](https://github.com/rustsec/audit-action) - Security auditing

## Troubleshooting

### Cache Misses

If caching isn't working effectively:

1. Check the cache key includes all relevant factors
2. Ensure `Cargo.lock` is committed to the repository
3. Verify the cache is being saved and restored correctly

### MSRV Failures

If the MSRV check fails:

1. Verify your `rust-version` in `Cargo.toml` is correct
2. Check that all dependencies support your declared MSRV
3. Consider raising the MSRV if dependencies require it

### Fixture Validation Errors

If the adapter harness job fails on fixture validation:

1. Ensure JSON fixtures are valid (use `python3 -m json.tool` to verify)
2. For JSONL files, each line must be valid JSON
3. Check file encodings are UTF-8

### Cross-Platform Issues

If tests pass on one platform but fail on another:

1. Check for path handling differences (use `std::path::Path`)
2. Verify line ending handling in test fixtures
3. Look for platform-specific dependencies

## Example: Minimal Workflow

For a minimal CI setup, use this stripped-down version:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test
      - run: cargo clippy -- -D warnings

  fmt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --check
```

## Best Practices

1. **Run CI on all PRs** - Catch issues before merging
2. **Fail fast on warnings** - Use `-D warnings` with clippy
3. **Test on all platforms** - Ensure cross-platform compatibility
4. **Cache dependencies** - Speed up CI runs significantly
5. **Validate fixtures** - Ensure test data is well-formed
6. **Check documentation** - Prevent doc build failures

## Related Documentation

- [How to Write an Adapter](./write-adapter.md)
- [How to Publish an Adapter](./publish-adapter.md)
- [How to Troubleshoot](./troubleshoot.md)
- [Adapter SDK Documentation](../../buildfix-adapter-sdk/README.md)
