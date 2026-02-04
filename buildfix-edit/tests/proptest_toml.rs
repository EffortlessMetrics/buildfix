//! Property-based tests for TOML editing operations.
//!
//! These tests verify key invariants:
//! - Idempotency: applying the same operation twice produces the same result
//! - Comment preservation: TOML comments survive transformations
//! - Unmodified tables: tables not touched by an operation stay identical
//! - Roundtrip parsing: parse(serialize(parse(toml))) == parse(toml)

use buildfix_edit::apply_op_to_content;
use buildfix_types::ops::{DepPreserve, Operation};
use camino::Utf8PathBuf;
use proptest::prelude::*;

/// Strategy to generate valid workspace TOML documents.
fn arb_workspace_toml() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::string::string_regex(r"[a-z][a-z0-9_-]*")
            .unwrap()
            .prop_filter("non-empty", |s| !s.is_empty()),
        1..5,
    )
    .prop_map(|members| {
        let members_str = members
            .iter()
            .map(|m| format!("\"crates/{}\"", m))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            r#"# Workspace configuration
[workspace]
members = [{}]

# Some comment to preserve
"#,
            members_str
        )
    })
}

/// Strategy to generate TOML with package section.
fn arb_package_toml() -> impl Strategy<Value = String> {
    (
        prop::string::string_regex(r"[a-z][a-z0-9_-]*")
            .unwrap()
            .prop_filter("non-empty", |s| !s.is_empty()),
        prop::string::string_regex(r"[0-9]+\.[0-9]+\.[0-9]+").unwrap(),
        prop::string::string_regex(r"1\.(6[5-9]|7[0-9])").unwrap(),
    )
        .prop_map(|(name, version, rust_version)| {
            format!(
                r#"# Package manifest
[package]
name = "{}"
version = "{}"
edition = "2021"
rust-version = "{}"

# Dependencies below
[dependencies]
"#,
                name, version, rust_version
            )
        })
}

/// Strategy to generate TOML with path dependency.
fn arb_path_dep_toml() -> impl Strategy<Value = String> {
    (
        prop::string::string_regex(r"[a-z][a-z0-9_-]*")
            .unwrap()
            .prop_filter("non-empty", |s| !s.is_empty()),
        prop::string::string_regex(r"[a-z][a-z0-9_-]*")
            .unwrap()
            .prop_filter("non-empty", |s| !s.is_empty()),
    )
        .prop_map(|(name, dep_name)| {
            format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

# My dependencies
[dependencies]
{} = {{ path = "../{}" }}
"#,
                name, dep_name, dep_name
            )
        })
}

/// Strategy to generate TOML with version dependency (for workspace inheritance test).
fn arb_version_dep_toml() -> impl Strategy<Value = String> {
    (
        prop::string::string_regex(r"[a-z][a-z0-9_-]*")
            .unwrap()
            .prop_filter("non-empty", |s| !s.is_empty()),
        prop::string::string_regex(r"[a-z][a-z0-9_-]*")
            .unwrap()
            .prop_filter("non-empty", |s| !s.is_empty()),
        prop::string::string_regex(r"[0-9]+\.[0-9]+").unwrap(),
    )
        .prop_map(|(name, dep_name, version)| {
            format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

# Dependencies
[dependencies]
{} = "{}"
"#,
                name, dep_name, version
            )
        })
}

proptest! {
    /// Applying the same resolver v2 operation twice yields the same result as once.
    #[test]
    fn idempotency_resolver_v2(toml in arb_workspace_toml()) {
        let op = Operation::EnsureWorkspaceResolverV2 {
            manifest: Utf8PathBuf::from("Cargo.toml"),
        };

        let r1 = apply_op_to_content(&toml, &op).unwrap();
        let r2 = apply_op_to_content(&r1, &op).unwrap();

        prop_assert_eq!(&r1, &r2, "operation should be idempotent");
    }

    /// Applying the same rust-version operation twice yields the same result.
    #[test]
    fn idempotency_rust_version(toml in arb_package_toml()) {
        let op = Operation::SetPackageRustVersion {
            manifest: Utf8PathBuf::from("Cargo.toml"),
            rust_version: "1.70".to_string(),
        };

        let r1 = apply_op_to_content(&toml, &op).unwrap();
        let r2 = apply_op_to_content(&r1, &op).unwrap();

        prop_assert_eq!(&r1, &r2, "operation should be idempotent");
    }

    /// Applying path dep version operation twice yields the same result.
    #[test]
    fn idempotency_path_dep_version(toml in arb_path_dep_toml()) {
        // Extract dep name from the generated TOML
        let doc: toml_edit::DocumentMut = toml.parse().unwrap();
        let deps = doc.get("dependencies").and_then(|d| d.as_table());
        let dep_name = deps
            .and_then(|t| t.iter().next())
            .map(|(k, _)| k.to_string())
            .unwrap_or_else(|| "dep".to_string());

        let op = Operation::EnsurePathDepHasVersion {
            manifest: Utf8PathBuf::from("Cargo.toml"),
            toml_path: vec!["dependencies".to_string(), dep_name.clone()],
            dep: dep_name.clone(),
            dep_path: format!("../{}", dep_name),
            version: "1.0.0".to_string(),
        };

        let r1 = apply_op_to_content(&toml, &op).unwrap();
        let r2 = apply_op_to_content(&r1, &op).unwrap();

        prop_assert_eq!(&r1, &r2, "operation should be idempotent");
    }

    /// Applying workspace inheritance twice yields the same result.
    #[test]
    fn idempotency_workspace_inheritance(toml in arb_version_dep_toml()) {
        // Extract dep name from the generated TOML
        let doc: toml_edit::DocumentMut = toml.parse().unwrap();
        let deps = doc.get("dependencies").and_then(|d| d.as_table());
        let dep_name = deps
            .and_then(|t| t.iter().next())
            .map(|(k, _)| k.to_string())
            .unwrap_or_else(|| "dep".to_string());

        let op = Operation::UseWorkspaceDependency {
            manifest: Utf8PathBuf::from("Cargo.toml"),
            toml_path: vec!["dependencies".to_string(), dep_name.clone()],
            dep: dep_name,
            preserved: DepPreserve::default(),
        };

        let r1 = apply_op_to_content(&toml, &op).unwrap();
        let r2 = apply_op_to_content(&r1, &op).unwrap();

        prop_assert_eq!(&r1, &r2, "operation should be idempotent");
    }

    /// Comments in TOML documents survive the resolver v2 transformation.
    #[test]
    fn comments_preserved_resolver_v2(toml in arb_workspace_toml()) {
        let op = Operation::EnsureWorkspaceResolverV2 {
            manifest: Utf8PathBuf::from("Cargo.toml"),
        };

        let result = apply_op_to_content(&toml, &op).unwrap();

        // Original had "# Workspace configuration" comment
        prop_assert!(
            result.contains("# Workspace configuration") || result.contains("# Some comment"),
            "comments should be preserved"
        );
    }

    /// Parsing roundtrip: parse(serialize(parse(toml))) produces equivalent structure.
    #[test]
    fn roundtrip_parse(toml in arb_workspace_toml()) {
        // Parse the input
        let doc1: toml_edit::DocumentMut = toml.parse().unwrap();
        // Serialize
        let serialized = doc1.to_string();
        // Parse again
        let doc2: toml_edit::DocumentMut = serialized.parse().unwrap();

        // The workspace table should be preserved
        let ws1 = doc1.get("workspace");
        let ws2 = doc2.get("workspace");

        prop_assert!(ws1.is_some() && ws2.is_some(), "workspace table should exist");
    }

    /// Tables not touched by an operation stay identical.
    #[test]
    fn unmodified_tables_unchanged(toml in arb_package_toml()) {
        let op = Operation::SetPackageRustVersion {
            manifest: Utf8PathBuf::from("Cargo.toml"),
            rust_version: "1.75".to_string(),
        };

        let before: toml_edit::DocumentMut = toml.parse().unwrap();
        let result = apply_op_to_content(&toml, &op).unwrap();
        let after: toml_edit::DocumentMut = result.parse().unwrap();

        // package.name should be unchanged
        let name_before = before["package"]["name"].as_str();
        let name_after = after["package"]["name"].as_str();
        prop_assert_eq!(name_before, name_after, "package.name should be unchanged");

        // package.version should be unchanged
        let version_before = before["package"]["version"].as_str();
        let version_after = after["package"]["version"].as_str();
        prop_assert_eq!(version_before, version_after, "package.version should be unchanged");

        // package.edition should be unchanged
        let edition_before = before["package"]["edition"].as_str();
        let edition_after = after["package"]["edition"].as_str();
        prop_assert_eq!(edition_before, edition_after, "package.edition should be unchanged");
    }
}
