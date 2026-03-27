# buildfix Configuration Profiles

This directory contains example configuration profiles for buildfix. Each profile is a pre-configured `buildfix.toml` designed for common use cases.

## Quick Start

1. Choose a profile that matches your needs (see [Profile Selection Guide](#profile-selection-guide))
2. Copy it to your repository root:
   ```bash
   cp examples/profiles/<profile-name>.toml buildfix.toml
   ```
3. Run buildfix:
   ```bash
   cargo run -p buildfix -- plan    # Generate a plan
   cargo run -p buildfix -- apply --apply  # Apply changes
   ```

## Profile Overview

| Profile | Safe Ops | Guarded Ops | Unsafe Ops | Best For |
|---------|----------|-------------|------------|----------|
| [conservative](#conservative) | ✅ Auto | ❌ Blocked | ❌ Blocked | CI pipelines |
| [balanced](#balanced) | ✅ Auto | ⚠️ With flag | ❌ Blocked | Development workflows |
| [aggressive-but-reviewed](#aggressive-but-reviewed) | ✅ Auto | ⚠️ With flag | ⚠️ With flag | Manual maintenance |

### Safety Classes Explained

- **Safe**: Fully determined from repository truth, no side effects. Examples: resolver v2, path dep versions, workspace inheritance.
- **Guarded**: Deterministic but higher impact. May affect MSRV, edition, or license metadata. Requires `--allow-guarded` flag.
- **Unsafe**: Requires explicit parameters. Can break code (e.g., removing deps with implicit feature flags). Requires `--allow-unsafe` flag.

## Profiles

### conservative

**File:** [`conservative.toml`](conservative.toml)

The safest profile - only allows operations that cannot break builds.

**Characteristics:**
- ✅ Safe operations auto-apply
- ❌ Guarded operations blocked
- ❌ Unsafe operations blocked
- Low limits (20 ops, 10 files, 50KB patches)

**Best for:**
- CI/CD pipelines that must never fail
- Automated maintenance bots
- Teams with strict change control

**Usage:**
```bash
cp examples/profiles/conservative.toml buildfix.toml
cargo run -p buildfix -- apply --apply
```

**What it fixes:**
- Workspace resolver version 2
- Missing versions on path dependencies
- Workspace dependency inheritance
- Duplicate dependency versions

---

### balanced

**File:** [`balanced.toml`](balanced.toml)

Balances automation with review - safe ops auto-apply, guarded ops need a flag.

**Characteristics:**
- ✅ Safe operations auto-apply
- ⚠️ Guarded operations require `--allow-guarded`
- ❌ Unsafe operations blocked
- Moderate limits (50 ops, 25 files, 250KB patches)

**Best for:**
- Development workflows
- Periodic maintenance runs
- Teams that review changes before merging

**Usage:**
```bash
cp examples/profiles/balanced.toml buildfix.toml

# Apply safe fixes only
cargo run -p buildfix -- apply --apply

# Apply safe + guarded fixes (review plan first!)
cargo run -p buildfix -- apply --apply --allow-guarded
```

**What it fixes (safe):**
- Everything from conservative profile

**What it fixes (guarded):**
- MSRV normalization across workspace
- Edition normalization across workspace
- License field normalization

---

### aggressive-but-reviewed

**File:** [`aggressive-but-reviewed.toml`](aggressive-but-reviewed.toml)

Enables all fixers but requires explicit flags for guarded/unsafe operations. Includes comprehensive documentation.

**Characteristics:**
- ✅ Safe operations auto-apply
- ⚠️ Guarded operations require `--allow-guarded`
- ⚠️ Unsafe operations require `--allow-unsafe`
- High limits (100 ops, 50 files, 500KB patches)
- Includes fixer reference documentation

**Best for:**
- Manual maintenance sessions
- Onboarding new repositories
- Comprehensive hygiene fixes with careful review

**Usage:**
```bash
cp examples/profiles/aggressive-but-reviewed.toml buildfix.toml

# Generate and review plan
cargo run -p buildfix -- plan
# Review plan.md and patch.diff carefully!

# Apply in stages
cargo run -p buildfix -- apply --apply                        # Safe only
cargo run -p buildfix -- apply --apply --allow-guarded        # + guarded
cargo run -p buildfix -- apply --apply --allow-guarded --allow-unsafe  # All
```

**What it fixes (everything):**
- All safe and guarded operations
- Unused dependency removal (unsafe) - **requires careful review!**

---

## Profile Selection Guide

### Decision Tree

```
Is this for automated CI?
├── YES → Will it run without human review?
│   ├── YES → Use conservative
│   └── NO → Use balanced (reviewer checks plan before merge)
└── NO → Is this a manual maintenance session?
    ├── YES → Do you want to fix everything?
    │   ├── YES → Use aggressive-but-reviewed
    │   └── NO → Use balanced
    └── NO → Use balanced (good default for most teams)
```

### By Scenario

| Scenario | Recommended Profile |
|----------|---------------------|
| GitHub Actions (auto-merge) | conservative |
| GitLab CI (with human approval) | balanced |
| Pre-commit hook | conservative |
| Weekly maintenance script | balanced |
| Onboarding a legacy codebase | aggressive-but-reviewed |
| Open source project | balanced |
| Internal enterprise project | balanced or aggressive-but-reviewed |

## Customizing Profiles

### Adding to Allowlist

To allow additional fixers, add their policy keys to the `allow` list:

```toml
[policy]
allow = [
  "builddiag/workspace.resolver_v2/*",
  # Add more fixers here
  "builddiag/rust.msrv_consistent/*",
]
```

### Denying Specific Fixers

To block specific fixers even when they would otherwise be allowed:

```toml
[policy]
deny = [
  "builddiag/rust.msrv_consistent/*",  # Never touch MSRV
]
```

### Adjusting Limits

Modify operational caps based on your project size:

```toml
[policy]
max_ops = 100         # Increase for large workspaces
max_files = 50        # Increase for many crates
max_patch_bytes = 1000000  # Increase for comprehensive changes
```

### Adding Parameters

For unsafe operations that require explicit values:

```toml
[params]
rust_version = "1.75"  # Target MSRV for normalization
```

### Disabling Backups

Not recommended, but available for CI environments with other backup strategies:

```toml
[backups]
enabled = false
```

## Further Reading

- [Configuration Guide](../../docs/how-to/configure.md) - Full configuration documentation
- [Configuration Schema](../../docs/config.md) - Schema reference
- [Fix Catalog](../../docs/reference/fixes.md) - Complete list of fixers and their policy keys

## Contributing

If you create a profile that works well for your use case, consider contributing it back! Profiles should:

1. Have a clear, descriptive name
2. Include comprehensive comments explaining the choices
3. Document when to use the profile
4. List what operations are enabled/blocked
