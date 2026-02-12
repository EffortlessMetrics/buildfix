#![no_main]

//! Fuzz target for operation application to TOML content.
//!
//! This fuzzes each operation type with arbitrary TOML inputs to ensure
//! the TOML editing code handles malformed input gracefully.

use buildfix_edit::apply_op_to_content;
use buildfix_types::ops::OpKind;
use libfuzzer_sys::fuzz_target;

/// Simple structured input for the fuzzer to generate diverse operation scenarios.
#[derive(Debug, arbitrary::Arbitrary)]
struct FuzzInput {
    /// Raw bytes to interpret as TOML content.
    toml_content: Vec<u8>,
    /// Which operation type to apply.
    op_type: OpType,
    /// Parameters for operations.
    dep_name: String,
    dep_path: String,
    version: String,
    rust_version: String,
    /// TOML path segments.
    toml_path_segments: Vec<String>,
    /// Optional features for preserve.
    optional: Option<bool>,
    default_features: Option<bool>,
    package: Option<String>,
    features: Vec<String>,
}

#[derive(Debug, arbitrary::Arbitrary)]
enum OpType {
    EnsureWorkspaceResolverV2,
    SetPackageRustVersion,
    EnsurePathDepHasVersion,
    UseWorkspaceDependency,
}

fuzz_target!(|input: FuzzInput| {
    let Ok(toml_str) = std::str::from_utf8(&input.toml_content) else {
        return;
    };

    let op = match input.op_type {
        OpType::EnsureWorkspaceResolverV2 => OpKind::TomlTransform {
            rule_id: "ensure_workspace_resolver_v2".to_string(),
            args: None,
        },
        OpType::SetPackageRustVersion => OpKind::TomlTransform {
            rule_id: "set_package_rust_version".to_string(),
            args: Some(serde_json::json!({
                "rust_version": input.rust_version,
            })),
        },
        OpType::EnsurePathDepHasVersion => {
            let toml_path = if input.toml_path_segments.is_empty() {
                vec!["dependencies".to_string(), input.dep_name.clone()]
            } else {
                input.toml_path_segments
            };
            OpKind::TomlTransform {
                rule_id: "ensure_path_dep_has_version".to_string(),
                args: Some(serde_json::json!({
                    "toml_path": toml_path,
                    "dep": input.dep_name,
                    "dep_path": input.dep_path,
                    "version": input.version,
                })),
            }
        }
        OpType::UseWorkspaceDependency => {
            let toml_path = if input.toml_path_segments.is_empty() {
                vec!["dependencies".to_string(), input.dep_name.clone()]
            } else {
                input.toml_path_segments
            };
            OpKind::TomlTransform {
                rule_id: "use_workspace_dependency".to_string(),
                args: Some(serde_json::json!({
                    "toml_path": toml_path,
                    "preserved": {
                        "package": input.package,
                        "optional": input.optional,
                        "default_features": input.default_features,
                        "features": input.features,
                    }
                })),
            }
        }
    };

    let _ = apply_op_to_content(toml_str, &op);
});
