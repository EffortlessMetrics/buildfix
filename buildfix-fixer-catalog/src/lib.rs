use buildfix_types::ops::SafetyClass;

/// Trigger pattern for a fixer.
#[derive(Debug, Clone, Copy)]
pub struct TriggerPattern {
    /// Sensor/tool name (e.g., "builddiag", "depguard").
    pub sensor: &'static str,
    /// Check ID pattern (e.g., "workspace.resolver_v2").
    pub check_id: &'static str,
    /// Optional code pattern (e.g., "missing_version").
    pub code: Option<&'static str>,
}

/// Canonical fix metadata shared between planner and explain surfaces.
#[derive(Debug, Clone, Copy)]
pub struct FixerCatalogEntry {
    /// User-facing CLI key (e.g., "resolver-v2").
    pub key: &'static str,
    /// Internal fix key used by planner policy (e.g., "cargo.workspace_resolver_v2").
    pub fix_id: &'static str,
    /// Safety class for planner policy output.
    pub safety: SafetyClass,
    /// Triggering sensor/check_id/code triplets.
    pub triggers: &'static [TriggerPattern],
}

#[cfg(feature = "fixer-resolver-v2")]
const RESOLVER_V2_TRIGGERS: &[TriggerPattern] = &[
    TriggerPattern {
        sensor: "builddiag",
        check_id: "workspace.resolver_v2",
        code: None,
    },
    TriggerPattern {
        sensor: "cargo",
        check_id: "cargo.workspace.resolver_v2",
        code: None,
    },
];

#[cfg(feature = "fixer-path-dep-version")]
const PATH_DEP_VERSION_TRIGGERS: &[TriggerPattern] = &[
    TriggerPattern {
        sensor: "depguard",
        check_id: "deps.path_requires_version",
        code: Some("missing_version"),
    },
    TriggerPattern {
        sensor: "depguard",
        check_id: "cargo.path_requires_version",
        code: Some("missing_version"),
    },
];

#[cfg(feature = "fixer-workspace-inheritance")]
const WORKSPACE_INHERITANCE_TRIGGERS: &[TriggerPattern] = &[
    TriggerPattern {
        sensor: "depguard",
        check_id: "deps.workspace_inheritance",
        code: None,
    },
    TriggerPattern {
        sensor: "depguard",
        check_id: "cargo.workspace_inheritance",
        code: None,
    },
];

#[cfg(feature = "fixer-duplicate-deps")]
const DUPLICATE_DEPS_TRIGGERS: &[TriggerPattern] = &[
    TriggerPattern {
        sensor: "depguard",
        check_id: "deps.duplicate_dependency_versions",
        code: None,
    },
    TriggerPattern {
        sensor: "depguard",
        check_id: "cargo.duplicate_dependency_versions",
        code: None,
    },
    TriggerPattern {
        sensor: "depguard",
        check_id: "deps.duplicate_versions",
        code: None,
    },
    TriggerPattern {
        sensor: "depguard",
        check_id: "cargo.duplicate_versions",
        code: None,
    },
];

#[cfg(feature = "fixer-remove-unused-deps")]
const REMOVE_UNUSED_DEPS_TRIGGERS: &[TriggerPattern] = &[
    TriggerPattern {
        sensor: "cargo-udeps",
        check_id: "deps.unused_dependency",
        code: None,
    },
    TriggerPattern {
        sensor: "udeps",
        check_id: "deps.unused_dependency",
        code: None,
    },
    TriggerPattern {
        sensor: "cargo-machete",
        check_id: "deps.unused_dependency",
        code: None,
    },
    TriggerPattern {
        sensor: "machete",
        check_id: "deps.unused_dependency",
        code: None,
    },
    TriggerPattern {
        sensor: "cargo-udeps",
        check_id: "deps.unused_dependencies",
        code: None,
    },
    TriggerPattern {
        sensor: "cargo-udeps",
        check_id: "cargo.unused_dependency",
        code: None,
    },
    TriggerPattern {
        sensor: "cargo-udeps",
        check_id: "cargo.unused_dependencies",
        code: None,
    },
    TriggerPattern {
        sensor: "udeps",
        check_id: "udeps.unused_dependency",
        code: None,
    },
    TriggerPattern {
        sensor: "machete",
        check_id: "machete.unused_dependency",
        code: None,
    },
];

#[cfg(feature = "fixer-msrv")]
const MSRV_TRIGGERS: &[TriggerPattern] = &[
    TriggerPattern {
        sensor: "builddiag",
        check_id: "rust.msrv_consistent",
        code: None,
    },
    TriggerPattern {
        sensor: "cargo",
        check_id: "cargo.msrv_consistent",
        code: None,
    },
    TriggerPattern {
        sensor: "cargo",
        check_id: "msrv.consistent",
        code: None,
    },
];

#[cfg(feature = "fixer-edition")]
const EDITION_TRIGGERS: &[TriggerPattern] = &[
    TriggerPattern {
        sensor: "builddiag",
        check_id: "rust.edition_consistent",
        code: None,
    },
    TriggerPattern {
        sensor: "cargo",
        check_id: "cargo.edition_consistent",
        code: None,
    },
    TriggerPattern {
        sensor: "cargo",
        check_id: "edition.consistent",
        code: None,
    },
];

#[cfg(feature = "fixer-license")]
const LICENSE_TRIGGERS: &[TriggerPattern] = &[
    TriggerPattern {
        sensor: "cargo-deny",
        check_id: "licenses.unlicensed",
        code: None,
    },
    TriggerPattern {
        sensor: "cargo-deny",
        check_id: "licenses.missing_license",
        code: None,
    },
    TriggerPattern {
        sensor: "deny",
        check_id: "licenses.unlicensed",
        code: None,
    },
    TriggerPattern {
        sensor: "deny",
        check_id: "licenses.missing_license",
        code: None,
    },
    TriggerPattern {
        sensor: "cargo-deny",
        check_id: "licenses.missing",
        code: None,
    },
    TriggerPattern {
        sensor: "deny",
        check_id: "license.unlicensed",
        code: None,
    },
    TriggerPattern {
        sensor: "deny",
        check_id: "license.missing",
        code: None,
    },
    TriggerPattern {
        sensor: "deny",
        check_id: "license.missing_license",
        code: None,
    },
    TriggerPattern {
        sensor: "cargo-deny",
        check_id: "cargo.licenses.unlicensed",
        code: None,
    },
    TriggerPattern {
        sensor: "cargo-deny",
        check_id: "cargo.licenses.missing_license",
        code: None,
    },
];

/// Returns all enabled builtins and their metadata.
#[allow(clippy::vec_init_then_push)]
pub fn enabled_fix_catalog() -> Vec<FixerCatalogEntry> {
    let mut out = Vec::new();

    #[cfg(feature = "fixer-resolver-v2")]
    out.push(FixerCatalogEntry {
        key: "resolver-v2",
        fix_id: "cargo.workspace_resolver_v2",
        safety: SafetyClass::Safe,
        triggers: RESOLVER_V2_TRIGGERS,
    });

    #[cfg(feature = "fixer-path-dep-version")]
    out.push(FixerCatalogEntry {
        key: "path-dep-version",
        fix_id: "cargo.path_dep_add_version",
        safety: SafetyClass::Safe,
        triggers: PATH_DEP_VERSION_TRIGGERS,
    });

    #[cfg(feature = "fixer-workspace-inheritance")]
    out.push(FixerCatalogEntry {
        key: "workspace-inheritance",
        fix_id: "cargo.use_workspace_dependency",
        safety: SafetyClass::Safe,
        triggers: WORKSPACE_INHERITANCE_TRIGGERS,
    });

    #[cfg(feature = "fixer-duplicate-deps")]
    out.push(FixerCatalogEntry {
        key: "duplicate-deps",
        fix_id: "cargo.consolidate_duplicate_deps",
        safety: SafetyClass::Safe,
        triggers: DUPLICATE_DEPS_TRIGGERS,
    });

    #[cfg(feature = "fixer-remove-unused-deps")]
    out.push(FixerCatalogEntry {
        key: "remove-unused-deps",
        fix_id: "cargo.remove_unused_deps",
        safety: SafetyClass::Unsafe,
        triggers: REMOVE_UNUSED_DEPS_TRIGGERS,
    });

    #[cfg(feature = "fixer-msrv")]
    out.push(FixerCatalogEntry {
        key: "msrv",
        fix_id: "cargo.normalize_rust_version",
        safety: SafetyClass::Guarded,
        triggers: MSRV_TRIGGERS,
    });

    #[cfg(feature = "fixer-edition")]
    out.push(FixerCatalogEntry {
        key: "edition",
        fix_id: "cargo.normalize_edition",
        safety: SafetyClass::Guarded,
        triggers: EDITION_TRIGGERS,
    });

    #[cfg(feature = "fixer-license")]
    out.push(FixerCatalogEntry {
        key: "license",
        fix_id: "cargo.normalize_license",
        safety: SafetyClass::Guarded,
        triggers: LICENSE_TRIGGERS,
    });

    out
}

/// Enabled fix IDs for interoperability checks.
pub fn enabled_fix_ids() -> Vec<&'static str> {
    enabled_fix_catalog()
        .iter()
        .map(|entry| entry.fix_id)
        .collect()
}

/// Enabled CLI keys for interoperability checks.
pub fn enabled_fix_keys() -> Vec<&'static str> {
    enabled_fix_catalog()
        .iter()
        .map(|entry| entry.key)
        .collect()
}

/// Find a catalog entry by key, fix_id, or suffix.
pub fn lookup_fix(query: &str) -> Option<FixerCatalogEntry> {
    let query_lower = query.to_lowercase();
    let query_normalized = query_lower.replace('_', "-");

    for entry in enabled_fix_catalog() {
        if entry.key == query_normalized
            || entry.fix_id.to_lowercase() == query_lower
            || entry
                .fix_id
                .to_lowercase()
                .ends_with(&format!(".{}", query_lower))
            || entry.fix_id.to_lowercase().replace('_', "-") == query_normalized
        {
            return Some(entry);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_ids_are_unique_when_enabled() {
        let ids = enabled_fix_ids();
        let unique: HashSet<_> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn lookup_is_case_insensitive_and_handles_underscores() {
        #[cfg(feature = "fixer-license")]
        {
            let by_key = lookup_fix("LICENSE").expect("license by key");
            assert_eq!(by_key.key, "license");

            let by_underscored = lookup_fix("normalize_license").expect("normalize_license");
            assert_eq!(by_underscored.key, "license");
        }

        #[cfg(feature = "fixer-workspace-inheritance")]
        {
            let by_suffix =
                lookup_fix("use_workspace_dependency").expect("workspace inheritance by suffix");
            assert_eq!(by_suffix.key, "workspace-inheritance");
        }
    }
}
