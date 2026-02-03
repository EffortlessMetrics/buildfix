#![no_main]

//! Fuzz target for operation application to TOML content.
//!
//! This fuzzes each operation type with arbitrary TOML inputs to ensure
//! the TOML editing code handles malformed input gracefully.

use libfuzzer_sys::fuzz_target;
use buildfix_types::ops::{DepPreserve, Operation};

/// Simple structured input for the fuzzer to generate diverse operation scenarios.
#[derive(Debug, arbitrary::Arbitrary)]
struct FuzzInput {
    /// Raw bytes to interpret as TOML content.
    toml_content: Vec<u8>,
    /// Which operation type to apply.
    op_type: OpType,
    /// Parameters for operations.
    manifest: String,
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
    // Parse the TOML content - don't proceed if not valid UTF-8.
    let Ok(toml_str) = std::str::from_utf8(&input.toml_content) else {
        return;
    };

    // Try to parse as TOML document - invalid TOML is fine, we just return early.
    let Ok(mut doc) = toml_str.parse::<toml_edit::DocumentMut>() else {
        return;
    };

    // Build the operation based on type.
    let manifest = camino::Utf8PathBuf::from(&input.manifest);

    let operation = match input.op_type {
        OpType::EnsureWorkspaceResolverV2 => {
            Operation::EnsureWorkspaceResolverV2 { manifest }
        }
        OpType::SetPackageRustVersion => {
            Operation::SetPackageRustVersion {
                manifest,
                rust_version: input.rust_version,
            }
        }
        OpType::EnsurePathDepHasVersion => {
            // Build a valid-ish toml_path.
            let toml_path = if input.toml_path_segments.is_empty() {
                vec!["dependencies".to_string(), input.dep_name.clone()]
            } else {
                input.toml_path_segments
            };

            Operation::EnsurePathDepHasVersion {
                manifest,
                toml_path,
                dep: input.dep_name,
                dep_path: input.dep_path,
                version: input.version,
            }
        }
        OpType::UseWorkspaceDependency => {
            let toml_path = if input.toml_path_segments.is_empty() {
                vec!["dependencies".to_string(), input.dep_name.clone()]
            } else {
                input.toml_path_segments
            };

            Operation::UseWorkspaceDependency {
                manifest,
                toml_path,
                dep: input.dep_name,
                preserved: DepPreserve {
                    package: input.package,
                    optional: input.optional,
                    default_features: input.default_features,
                    features: input.features,
                },
            }
        }
    };

    // Apply the operation using toml_edit - should never panic.
    apply_operation_to_doc(&mut doc, &operation);

    // Serialize back to string - should never panic.
    let _ = doc.to_string();
});

/// Apply an operation to a TOML document (mirrors buildfix-edit logic).
fn apply_operation_to_doc(doc: &mut toml_edit::DocumentMut, op: &Operation) {
    use toml_edit::{value, InlineTable, Item};

    match op {
        Operation::EnsureWorkspaceResolverV2 { .. } => {
            doc["workspace"]["resolver"] = value("2");
        }

        Operation::SetPackageRustVersion { rust_version, .. } => {
            doc["package"]["rust-version"] = value(rust_version.as_str());
        }

        Operation::EnsurePathDepHasVersion {
            toml_path,
            dep_path,
            version,
            ..
        } => {
            if let Some(dep_item) = get_dep_item_mut(doc, toml_path) {
                if let Some(inline) = dep_item.as_inline_table_mut() {
                    let current_path = inline.get("path").and_then(|v| v.as_str());
                    if current_path == Some(dep_path.as_str()) {
                        if inline.get("version").and_then(|v| v.as_str()).is_none() {
                            inline.insert("version", str_value(version));
                        }
                    }
                } else if let Some(tbl) = dep_item.as_table_mut() {
                    let current_path = tbl
                        .get("path")
                        .and_then(|i| i.as_value())
                        .and_then(|v| v.as_str());
                    if current_path == Some(dep_path.as_str()) {
                        if tbl
                            .get("version")
                            .and_then(|i| i.as_value())
                            .and_then(|v| v.as_str())
                            .is_none()
                        {
                            tbl["version"] = value(version.as_str());
                        }
                    }
                }
            }
        }

        Operation::UseWorkspaceDependency {
            toml_path,
            preserved,
            ..
        } => {
            if let Some(dep_item) = get_dep_item_mut(doc, toml_path) {
                let mut inline = InlineTable::new();
                inline.insert("workspace", bool_value(true));
                if let Some(pkg) = &preserved.package {
                    inline.insert("package", str_value(pkg));
                }
                if let Some(opt) = preserved.optional {
                    inline.insert("optional", bool_value(opt));
                }
                if let Some(df) = preserved.default_features {
                    inline.insert("default-features", bool_value(df));
                }
                if !preserved.features.is_empty() {
                    let mut arr = toml_edit::Array::new();
                    for f in &preserved.features {
                        arr.push(f.as_str());
                    }
                    inline.insert("features", value(arr).as_value().unwrap().clone());
                }
                *dep_item = value(inline);
            }
        }
    }
}

fn str_value(s: &str) -> toml_edit::Value {
    toml_edit::value(s).as_value().unwrap().clone()
}

fn bool_value(b: bool) -> toml_edit::Value {
    toml_edit::value(b).as_value().unwrap().clone()
}

fn get_dep_item_mut<'a>(
    doc: &'a mut toml_edit::DocumentMut,
    toml_path: &[String],
) -> Option<&'a mut toml_edit::Item> {
    if toml_path.len() < 2 {
        return None;
    }

    if toml_path[0] == "target" {
        if toml_path.len() < 4 {
            return None;
        }
        let cfg = &toml_path[1];
        let table_name = &toml_path[2];
        let dep = &toml_path[3];

        let target = doc.get_mut("target")?.as_table_mut()?;
        let cfg_tbl = target.get_mut(cfg)?.as_table_mut()?;
        let deps = cfg_tbl.get_mut(table_name)?.as_table_mut()?;
        return deps.get_mut(dep);
    }

    let table_name = &toml_path[0];
    let dep = &toml_path[1];
    let deps = doc.get_mut(table_name)?.as_table_mut()?;
    deps.get_mut(dep)
}
