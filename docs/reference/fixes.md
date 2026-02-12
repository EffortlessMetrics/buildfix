# Fix Catalog

Complete reference of all buildfix fixes, their triggers, safety classes, and behavior.

## Overview

| Fix | Key | Safety | Description |
|-----|-----|--------|-------------|
| [Workspace Resolver V2](#workspace-resolver-v2) | `resolver-v2` | Safe | Set resolver = "2" |
| [Path Dependency Version](#path-dependency-version) | `path-dep-version` | Safe | Add version to path deps |
| [Workspace Inheritance](#workspace-dependency-inheritance) | `workspace-inheritance` | Safe | Use workspace = true |
| [MSRV Normalization](#msrv-normalization) | `msrv` | Guarded | Normalize rust-version |
| [Edition Normalization](#edition-normalization) | `edition` | Guarded | Normalize edition |

## Workspace Resolver V2

**Key**: `resolver-v2`
**Fix ID**: `cargo.workspace_resolver_v2`
**Safety**: Safe

### Description

Sets `[workspace].resolver = "2"` in the root Cargo.toml.

Cargo's resolver v2 is the modern feature resolver that provides correct feature unification across the dependency graph. It prevents surprising behavior where dev-dependencies can enable features in normal dependencies.

### Triggering Findings

| Sensor | Check ID | Code |
|--------|----------|------|
| builddiag | workspace.resolver_v2 | * |
| cargo | cargo.workspace.resolver_v2 | * |

### Example Edit

```diff
 [workspace]
 members = ["crates/*"]
+resolver = "2"
```

### Preconditions

- File must be a workspace (has `[workspace]` table)
- File must be valid TOML

### Policy Keys

```
builddiag/workspace.resolver_v2/*
cargo/cargo.workspace_resolver_v2/*
```

---

## Path Dependency Version

**Key**: `path-dep-version`
**Fix ID**: `cargo.path_dep_add_version`
**Safety**: Safe

### Description

Adds a `version` field to path dependencies that are missing one.

Path dependencies without a version field cannot be published to crates.io. This fix automatically determines the correct version by reading the target crate's Cargo.toml.

### Triggering Findings

| Sensor | Check ID | Code |
|--------|----------|------|
| depguard | deps.path_requires_version | missing_version |
| depguard | cargo.path_requires_version | missing_version |

### Example Edit

```diff
 [dependencies]
-foo = { path = "../foo" }
+foo = { path = "../foo", version = "1.0.0" }
```

### Preconditions

- Target crate's Cargo.toml exists
- Target crate has `[package].version` defined
- Only one possible target (no ambiguity)

### Safety Notes

If the version cannot be determined (target missing or ambiguous), the fix is skipped to maintain safety.

### Policy Keys

```
depguard/deps.path_requires_version/missing_version
depguard/cargo.path_requires_version/missing_version
```

---

## Workspace Dependency Inheritance

**Key**: `workspace-inheritance`
**Fix ID**: `cargo.use_workspace_dependency`
**Safety**: Safe

### Description

Converts member crate dependencies to use workspace inheritance (`{ workspace = true }`).

When a dependency is defined in `[workspace.dependencies]`, member crates should use inheritance instead of specifying the version directly. This ensures version consistency across the workspace.

### Triggering Findings

| Sensor | Check ID | Code |
|--------|----------|------|
| depguard | deps.workspace_inheritance | * |
| depguard | cargo.workspace_inheritance | * |

### Example Edit

```diff
 [dependencies]
-serde = "1.0"
+serde = { workspace = true }
```

With preserved overrides:

```diff
 [dependencies]
-serde = { version = "1.0", features = ["derive"] }
+serde = { workspace = true, features = ["derive"] }
```

### Preserved Keys

The fix preserves these per-crate overrides:

- `features` — Additional features for this crate
- `optional` — Whether the dependency is optional
- `default-features` — Whether to include default features
- `package` — Renamed dependencies

### Preconditions

- Dependency exists in `[workspace.dependencies]`
- Not already using `workspace = true`
- Not a path or git dependency

### Policy Keys

```
depguard/deps.workspace_inheritance/*
depguard/cargo.workspace_inheritance/*
```

---

## MSRV Normalization

**Key**: `msrv`
**Fix ID**: `cargo.normalize_rust_version`
**Safety**: Guarded

### Description

Normalizes per-crate `rust-version` (MSRV) declarations to match the workspace canonical value.

The canonical rust-version is determined from:
1. `[workspace.package].rust-version` in root Cargo.toml
2. `[package].rust-version` in root Cargo.toml

### Triggering Findings

| Sensor | Check ID | Code |
|--------|----------|------|
| builddiag | rust.msrv_consistent | * |
| cargo | cargo.msrv_consistent | * |
| cargo | msrv.consistent | * |

### Example Edit

```diff
 [package]
 name = "my-crate"
 version = "0.1.0"
-rust-version = "1.65"
+rust-version = "1.70"
```

### Why Guarded?

This fix is classified as **Guarded** because:

- Changing MSRV affects which Rust versions can compile the crate
- A lower MSRV might hide newer Rust features being used
- A higher MSRV might break builds for users on older toolchains

Manual review is recommended before applying.

### Apply Command

```bash
buildfix apply --apply --allow-guarded
```

### Preconditions

- Workspace has a canonical rust-version defined
- Member crate has a different rust-version

### Skip Conditions

Fix is skipped when:
- No canonical workspace rust-version exists
- Crate already has the correct rust-version

### Policy Keys

```
builddiag/rust.msrv_consistent/*
cargo/cargo.msrv_consistent/*
cargo/msrv.consistent/*
```

---

## Edition Normalization

**Key**: `edition`
**Fix ID**: `cargo.normalize_edition`
**Safety**: Guarded (Unsafe when no workspace edition defined)

### Description

Normalizes per-crate `edition` declarations to match the workspace canonical value.

The canonical edition is determined from:
1. `[workspace.package].edition` in root Cargo.toml
2. `[package].edition` in root Cargo.toml

When no canonical edition can be determined, the fix is classified as **Unsafe** and requires the user to provide the edition via `--param edition=<value>`.

### Triggering Findings

| Sensor | Check ID | Code |
|--------|----------|------|
| builddiag | rust.edition_consistent | * |
| cargo | cargo.edition_consistent | * |
| cargo | edition.consistent | * |

### Example Edit

```diff
 [package]
 name = "my-crate"
 version = "0.1.0"
-edition = "2018"
+edition = "2021"
```

### Why Guarded?

This fix is classified as **Guarded** because:

- Changing the edition affects language semantics and available syntax
- A newer edition may introduce breaking changes (e.g., `dyn Trait` defaults, macro changes)
- A downgrade could cause compilation failures if newer edition features are in use

Manual review is recommended before applying.

### Apply Command

```bash
# When workspace edition is defined:
buildfix apply --apply --allow-guarded

# When no workspace edition exists:
buildfix apply --apply --allow-unsafe --param edition=2021
```

### Preconditions

- Member crate has an `edition` field that differs from the workspace canonical value
- Target file is valid TOML with a `[package]` table

### Skip Conditions

Fix is skipped when:
- No triggering findings exist
- Crate already has the correct edition

### Policy Keys

```
builddiag/rust.edition_consistent/*
cargo/cargo.edition_consistent/*
cargo/edition.consistent/*
```

---

## Policy Key Patterns

Policy matching supports patterns:

| Pattern | Matches |
|---------|---------|
| `sensor/*` | All findings from sensor |
| `sensor/check_id/*` | All codes for check |
| `sensor/check_id/code` | Exact match |

### Examples

```toml
[policy]
# Allow all depguard findings
allow = ["depguard/*"]

# Deny MSRV findings
deny = ["builddiag/rust.msrv_consistent/*"]

# Allow specific fix
allow = ["depguard/deps.path_requires_version/missing_version"]
```

## See Also

- [CLI Reference](cli.md) — `buildfix explain` command
- [Safety Model](../safety-model.md) — Safe/guarded/unsafe classification
- [Configuration](config.md) — Allow/deny policies
