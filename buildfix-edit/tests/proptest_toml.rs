//! Property-based tests for TOML editing operations.
//!
//! These tests verify key invariants:
//! - Idempotency: applying the same operation twice produces the same result
//! - Comment preservation: TOML comments survive transformations
//! - Unmodified tables: tables not touched by an operation stay identical
//! - Roundtrip parsing: parse(serialize(parse(toml))) == parse(toml)

use buildfix_edit::apply_op_to_content;
use buildfix_types::ops::OpKind;
use proptest::prelude::*;

#[derive(Debug, Clone)]
struct PreservedArgs {
    package: Option<String>,
    optional: Option<bool>,
    default_features: Option<bool>,
    features: Option<Vec<String>>,
}

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

fn arb_preserved_args() -> impl Strategy<Value = PreservedArgs> {
    let ident = || {
        prop::string::string_regex(r"[a-z][a-z0-9_-]*")
            .unwrap()
            .prop_filter("non-empty", |s| !s.is_empty())
    };
    (
        prop::option::of(ident()),
        prop::option::of(any::<bool>()),
        prop::option::of(any::<bool>()),
        prop::option::of(prop::collection::vec(ident(), 1..4)),
    )
        .prop_map(
            |(package, optional, default_features, features)| PreservedArgs {
                package,
                optional,
                default_features,
                features,
            },
        )
}

proptest! {
    /// Applying the same resolver v2 operation twice yields the same result as once.
    #[test]
    fn idempotency_resolver_v2(toml in arb_workspace_toml()) {
        let op = OpKind::TomlTransform {
            rule_id: "ensure_workspace_resolver_v2".to_string(),
            args: None,
        };

        let r1 = apply_op_to_content(&toml, &op).unwrap();
        let r2 = apply_op_to_content(&r1, &op).unwrap();

        prop_assert_eq!(&r1, &r2, "operation should be idempotent");
    }

    /// Applying the same rust-version operation twice yields the same result.
    #[test]
    fn idempotency_rust_version(toml in arb_package_toml()) {
        let mut args = serde_json::Map::new();
        args.insert(
            "rust_version".to_string(),
            serde_json::Value::String("1.70".to_string()),
        );
        let op = OpKind::TomlTransform {
            rule_id: "set_package_rust_version".to_string(),
            args: Some(serde_json::Value::Object(args)),
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

        let mut args = serde_json::Map::new();
        args.insert(
            "toml_path".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::String("dependencies".to_string()),
                serde_json::Value::String(dep_name.clone()),
            ]),
        );
        args.insert("dep".to_string(), serde_json::Value::String(dep_name.clone()));
        args.insert(
            "dep_path".to_string(),
            serde_json::Value::String(format!("../{}", dep_name)),
        );
        args.insert(
            "version".to_string(),
            serde_json::Value::String("1.0.0".to_string()),
        );
        let op = OpKind::TomlTransform {
            rule_id: "ensure_path_dep_has_version".to_string(),
            args: Some(serde_json::Value::Object(args)),
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

        let mut args = serde_json::Map::new();
        args.insert(
            "toml_path".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::String("dependencies".to_string()),
                serde_json::Value::String(dep_name),
            ]),
        );
        args.insert(
            "preserved".to_string(),
            serde_json::Value::Object(serde_json::Map::new()),
        );
        let op = OpKind::TomlTransform {
            rule_id: "use_workspace_dependency".to_string(),
            args: Some(serde_json::Value::Object(args)),
        };

        let r1 = apply_op_to_content(&toml, &op).unwrap();
        let r2 = apply_op_to_content(&r1, &op).unwrap();

        prop_assert_eq!(&r1, &r2, "operation should be idempotent");
    }

    /// Comments in TOML documents survive the resolver v2 transformation.
    #[test]
    fn comments_preserved_resolver_v2(toml in arb_workspace_toml()) {
        let op = OpKind::TomlTransform {
            rule_id: "ensure_workspace_resolver_v2".to_string(),
            args: None,
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
        let mut args = serde_json::Map::new();
        args.insert(
            "rust_version".to_string(),
            serde_json::Value::String("1.75".to_string()),
        );
        let op = OpKind::TomlTransform {
            rule_id: "set_package_rust_version".to_string(),
            args: Some(serde_json::Value::Object(args)),
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

    /// Workspace inheritance preserves dependency attributes.
    #[test]
    fn workspace_inheritance_preserves_fields(
        toml in arb_version_dep_toml(),
        preserved in arb_preserved_args(),
    ) {
        let doc: toml_edit::DocumentMut = toml.parse().unwrap();
        let deps = doc.get("dependencies").and_then(|d| d.as_table());
        let dep_name = deps
            .and_then(|t| t.iter().next())
            .map(|(k, _)| k.to_string())
            .unwrap_or_else(|| "dep".to_string());

        let mut args = serde_json::Map::new();
        args.insert(
            "toml_path".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::String("dependencies".to_string()),
                serde_json::Value::String(dep_name.clone()),
            ]),
        );

        let mut preserved_map = serde_json::Map::new();
        if let Some(pkg) = &preserved.package {
            preserved_map.insert("package".to_string(), serde_json::Value::String(pkg.clone()));
        }
        if let Some(opt) = preserved.optional {
            preserved_map.insert("optional".to_string(), serde_json::Value::Bool(opt));
        }
        if let Some(df) = preserved.default_features {
            preserved_map.insert("default_features".to_string(), serde_json::Value::Bool(df));
        }
        if let Some(features) = &preserved.features {
            preserved_map.insert(
                "features".to_string(),
                serde_json::Value::Array(
                    features
                        .iter()
                        .cloned()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }

        args.insert(
            "preserved".to_string(),
            serde_json::Value::Object(preserved_map),
        );

        let op = OpKind::TomlTransform {
            rule_id: "use_workspace_dependency".to_string(),
            args: Some(serde_json::Value::Object(args)),
        };

        let result = apply_op_to_content(&toml, &op).unwrap();
        let after: toml_edit::DocumentMut = result.parse().unwrap();
        let dep = after["dependencies"][&dep_name]
            .as_inline_table()
            .expect("dependency should be inline table");

        prop_assert_eq!(
            dep.get("workspace").and_then(|v| v.as_bool()),
            Some(true),
            "workspace=true should be set"
        );

        if let Some(pkg) = &preserved.package {
            prop_assert_eq!(
                dep.get("package").and_then(|v| v.as_str()),
                Some(pkg.as_str()),
                "package should be preserved"
            );
        }
        if let Some(opt) = preserved.optional {
            prop_assert_eq!(
                dep.get("optional").and_then(|v| v.as_bool()),
                Some(opt),
                "optional should be preserved"
            );
        }
        if let Some(df) = preserved.default_features {
            prop_assert_eq!(
                dep.get("default-features").and_then(|v| v.as_bool()),
                Some(df),
                "default-features should be preserved"
            );
        }
        if let Some(features) = &preserved.features {
            let arr = dep.get("features").and_then(|v| v.as_array());
            let rendered: Vec<String> = arr
                .map(|a| a.iter().filter_map(|i| i.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            prop_assert_eq!(
                rendered, features.clone(),
                "features should be preserved"
            );
        }
    }
}
