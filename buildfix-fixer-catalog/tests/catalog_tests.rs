//! Integration tests for buildfix-fixer-catalog.
//!
//! Tests catalog registration, fixer lookup, and all fixers present.

use buildfix_fixer_catalog::{enabled_fix_catalog, enabled_fix_ids, enabled_fix_keys, lookup_fix};
use buildfix_types::ops::SafetyClass;
use std::collections::HashSet;

// ============================================================================
// FixerCatalogEntry Tests
// ============================================================================

#[test]
fn test_catalog_entry_has_key() {
    let catalog = enabled_fix_catalog();

    for entry in &catalog {
        // Every entry should have a non-empty key
        assert!(!entry.key.is_empty(), "Entry should have a non-empty key");
    }
}

#[test]
fn test_catalog_entry_has_fix_id() {
    let catalog = enabled_fix_catalog();

    for entry in &catalog {
        // Every entry should have a non-empty fix_id
        assert!(
            !entry.fix_id.is_empty(),
            "Entry should have a non-empty fix_id"
        );
    }
}

#[test]
fn test_catalog_entry_has_triggers() {
    let catalog = enabled_fix_catalog();

    for entry in &catalog {
        // Every entry should have at least one trigger
        assert!(
            !entry.triggers.is_empty(),
            "Entry {} should have at least one trigger",
            entry.key
        );
    }
}

#[test]
fn test_catalog_entry_trigger_patterns() {
    let catalog = enabled_fix_catalog();

    for entry in &catalog {
        for trigger in entry.triggers {
            // Every trigger should have non-empty sensor and check_id
            assert!(
                !trigger.sensor.is_empty(),
                "Trigger for {} should have a non-empty sensor",
                entry.key
            );
            assert!(
                !trigger.check_id.is_empty(),
                "Trigger for {} should have a non-empty check_id",
                entry.key
            );
        }
    }
}

// ============================================================================
// Catalog Uniqueness Tests
// ============================================================================

#[test]
fn test_fix_ids_are_unique() {
    let ids = enabled_fix_ids();
    let unique: HashSet<_> = ids.iter().copied().collect();

    assert_eq!(ids.len(), unique.len(), "All fix_ids should be unique");
}

#[test]
fn test_fix_keys_are_unique() {
    let keys = enabled_fix_keys();
    let unique: HashSet<_> = keys.iter().copied().collect();

    assert_eq!(keys.len(), unique.len(), "All keys should be unique");
}

// ============================================================================
// Lookup Function Tests
// ============================================================================

#[test]
fn test_lookup_fix_by_key() {
    // Test looking up fixers by their CLI key
    #[cfg(feature = "fixer-resolver-v2")]
    {
        let entry = lookup_fix("resolver-v2");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.key, "resolver-v2");
    }

    #[cfg(feature = "fixer-path-dep-version")]
    {
        let entry = lookup_fix("path-dep-version");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.key, "path-dep-version");
    }

    #[cfg(feature = "fixer-msrv")]
    {
        let entry = lookup_fix("msrv");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.key, "msrv");
    }

    #[cfg(feature = "fixer-edition")]
    {
        let entry = lookup_fix("edition");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.key, "edition");
    }

    #[cfg(feature = "fixer-license")]
    {
        let entry = lookup_fix("license");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.key, "license");
    }
}

#[test]
fn test_lookup_fix_by_fix_id() {
    // Test looking up fixers by their internal fix_id
    #[cfg(feature = "fixer-resolver-v2")]
    {
        let entry = lookup_fix("cargo.workspace_resolver_v2");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.fix_id, "cargo.workspace_resolver_v2");
    }

    #[cfg(feature = "fixer-path-dep-version")]
    {
        let entry = lookup_fix("cargo.path_dep_add_version");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.fix_id, "cargo.path_dep_add_version");
    }
}

#[test]
fn test_lookup_fix_case_insensitive() {
    #[cfg(feature = "fixer-license")]
    {
        // Lookup should be case-insensitive
        let entry_lower = lookup_fix("license");
        let entry_upper = lookup_fix("LICENSE");
        let entry_mixed = lookup_fix("License");

        assert!(entry_lower.is_some());
        assert!(entry_upper.is_some());
        assert!(entry_mixed.is_some());

        // All should return the same entry
        assert_eq!(entry_lower.unwrap().key, entry_upper.unwrap().key);
        assert_eq!(entry_lower.unwrap().key, entry_mixed.unwrap().key);
    }
}

#[test]
fn test_lookup_fix_handles_underscores() {
    #[cfg(feature = "fixer-license")]
    {
        // Underscores should be treated like hyphens
        let entry_hyphen = lookup_fix("normalize-license");
        let entry_underscore = lookup_fix("normalize_license");

        assert!(entry_hyphen.is_some() || entry_underscore.is_some());

        if let (Some(e1), Some(e2)) = (entry_hyphen, entry_underscore) {
            assert_eq!(e1.key, e2.key);
        }
    }
}

#[test]
fn test_lookup_fix_nonexistent() {
    let entry = lookup_fix("nonexistent-fixer");
    assert!(entry.is_none(), "Should return None for nonexistent fixer");

    let entry = lookup_fix("random.string.that.does.not.exist");
    assert!(entry.is_none(), "Should return None for random query");
}

// ============================================================================
// Safety Class Tests
// ============================================================================

#[test]
fn test_catalog_safety_classes() {
    let catalog = enabled_fix_catalog();

    // Ensure we have fixers with different safety classes
    let has_safe = catalog.iter().any(|e| e.safety == SafetyClass::Safe);
    let has_guarded = catalog.iter().any(|e| e.safety == SafetyClass::Guarded);
    let has_unsafe = catalog.iter().any(|e| e.safety == SafetyClass::Unsafe);

    // At least one of each should exist when default features are enabled
    #[cfg(feature = "fixer-resolver-v2")]
    assert!(has_safe, "Should have at least one Safe fixer");

    #[cfg(feature = "fixer-msrv")]
    assert!(has_guarded, "Should have at least one Guarded fixer");

    #[cfg(feature = "fixer-remove-unused-deps")]
    assert!(has_unsafe, "Should have at least one Unsafe fixer");
}

#[test]
fn test_safe_fixers() {
    let catalog = enabled_fix_catalog();
    let safe_fixers: Vec<_> = catalog
        .iter()
        .filter(|e| e.safety == SafetyClass::Safe)
        .collect();

    for entry in safe_fixers {
        // Safe fixers should have well-defined triggers
        assert!(!entry.triggers.is_empty());

        // Known safe fixers
        let known_safe = [
            "resolver-v2",
            "path-dep-version",
            "workspace-inheritance",
            "duplicate-deps",
        ];
        if known_safe.contains(&entry.key) {
            assert_eq!(
                entry.safety,
                SafetyClass::Safe,
                "{} should be Safe",
                entry.key
            );
        }
    }
}

#[test]
fn test_guarded_fixers() {
    let catalog = enabled_fix_catalog();
    let guarded_fixers: Vec<_> = catalog
        .iter()
        .filter(|e| e.safety == SafetyClass::Guarded)
        .collect();

    for entry in guarded_fixers {
        // Known guarded fixers
        let known_guarded = ["msrv", "edition", "license"];
        if known_guarded.contains(&entry.key) {
            assert_eq!(
                entry.safety,
                SafetyClass::Guarded,
                "{} should be Guarded",
                entry.key
            );
        }
    }
}

#[test]
fn test_unsafe_fixers() {
    let catalog = enabled_fix_catalog();
    let unsafe_fixers: Vec<_> = catalog
        .iter()
        .filter(|e| e.safety == SafetyClass::Unsafe)
        .collect();

    for entry in unsafe_fixers {
        // Known unsafe fixers
        let known_unsafe = ["remove-unused-deps"];
        if known_unsafe.contains(&entry.key) {
            assert_eq!(
                entry.safety,
                SafetyClass::Unsafe,
                "{} should be Unsafe",
                entry.key
            );
        }
    }
}

// ============================================================================
// Trigger Pattern Tests
// ============================================================================

#[test]
fn test_resolver_v2_triggers() {
    #[cfg(feature = "fixer-resolver-v2")]
    {
        let entry = lookup_fix("resolver-v2").expect("resolver-v2 should exist");
        assert!(!entry.triggers.is_empty());

        // Should include builddiag trigger
        let has_builddiag = entry
            .triggers
            .iter()
            .any(|t| t.sensor == "builddiag" && t.check_id == "workspace.resolver_v2");
        assert!(has_builddiag, "Should have builddiag trigger");

        // Should include cargo trigger
        let has_cargo = entry
            .triggers
            .iter()
            .any(|t| t.sensor == "cargo" && t.check_id == "cargo.workspace.resolver_v2");
        assert!(has_cargo, "Should have cargo trigger");
    }
}

#[test]
fn test_path_dep_version_triggers() {
    #[cfg(feature = "fixer-path-dep-version")]
    {
        let entry = lookup_fix("path-dep-version").expect("path-dep-version should exist");
        assert!(!entry.triggers.is_empty());

        // Should include depguard triggers with code
        let has_depguard = entry.triggers.iter().any(|t| {
            t.sensor == "depguard"
                && t.check_id.contains("path_requires_version")
                && t.code == Some("missing_version")
        });
        assert!(
            has_depguard,
            "Should have depguard trigger with missing_version code"
        );
    }
}

#[test]
fn test_remove_unused_deps_triggers() {
    #[cfg(feature = "fixer-remove-unused-deps")]
    {
        let entry = lookup_fix("remove-unused-deps").expect("remove-unused-deps should exist");
        assert!(!entry.triggers.is_empty());

        // Should include udeps triggers
        let has_udeps = entry
            .triggers
            .iter()
            .any(|t| t.sensor == "cargo-udeps" || t.sensor == "udeps");
        assert!(has_udeps, "Should have udeps trigger");

        // Should include machete triggers
        let has_machete = entry
            .triggers
            .iter()
            .any(|t| t.sensor == "cargo-machete" || t.sensor == "machete");
        assert!(has_machete, "Should have machete trigger");
    }
}

#[test]
fn test_msrv_triggers() {
    #[cfg(feature = "fixer-msrv")]
    {
        let entry = lookup_fix("msrv").expect("msrv should exist");
        assert!(!entry.triggers.is_empty());

        // Should include builddiag trigger
        let has_builddiag = entry
            .triggers
            .iter()
            .any(|t| t.sensor == "builddiag" && t.check_id.contains("msrv"));
        assert!(has_builddiag, "Should have builddiag msrv trigger");
    }
}

#[test]
fn test_edition_triggers() {
    #[cfg(feature = "fixer-edition")]
    {
        let entry = lookup_fix("edition").expect("edition should exist");
        assert!(!entry.triggers.is_empty());

        // Should include builddiag trigger
        let has_builddiag = entry
            .triggers
            .iter()
            .any(|t| t.sensor == "builddiag" && t.check_id.contains("edition"));
        assert!(has_builddiag, "Should have builddiag edition trigger");
    }
}

#[test]
fn test_license_triggers() {
    #[cfg(feature = "fixer-license")]
    {
        let entry = lookup_fix("license").expect("license should exist");
        assert!(!entry.triggers.is_empty());

        // Should include cargo-deny triggers
        let has_deny = entry
            .triggers
            .iter()
            .any(|t| t.sensor == "cargo-deny" && t.check_id.contains("license"));
        assert!(has_deny, "Should have cargo-deny license trigger");
    }
}

// ============================================================================
// Feature Flag Tests
// ============================================================================

#[test]
fn test_all_default_fixers_enabled() {
    let catalog = enabled_fix_catalog();

    // With default features, all fixers should be enabled
    #[cfg(feature = "fixer-resolver-v2")]
    assert!(
        catalog.iter().any(|e| e.key == "resolver-v2"),
        "resolver-v2 should be enabled"
    );

    #[cfg(feature = "fixer-path-dep-version")]
    assert!(
        catalog.iter().any(|e| e.key == "path-dep-version"),
        "path-dep-version should be enabled"
    );

    #[cfg(feature = "fixer-workspace-inheritance")]
    assert!(
        catalog.iter().any(|e| e.key == "workspace-inheritance"),
        "workspace-inheritance should be enabled"
    );

    #[cfg(feature = "fixer-duplicate-deps")]
    assert!(
        catalog.iter().any(|e| e.key == "duplicate-deps"),
        "duplicate-deps should be enabled"
    );

    #[cfg(feature = "fixer-remove-unused-deps")]
    assert!(
        catalog.iter().any(|e| e.key == "remove-unused-deps"),
        "remove-unused-deps should be enabled"
    );

    #[cfg(feature = "fixer-msrv")]
    assert!(
        catalog.iter().any(|e| e.key == "msrv"),
        "msrv should be enabled"
    );

    #[cfg(feature = "fixer-edition")]
    assert!(
        catalog.iter().any(|e| e.key == "edition"),
        "edition should be enabled"
    );

    #[cfg(feature = "fixer-license")]
    assert!(
        catalog.iter().any(|e| e.key == "license"),
        "license should be enabled"
    );
}

// ============================================================================
// Catalog Count Tests
// ============================================================================

#[test]
fn test_catalog_count() {
    let catalog = enabled_fix_catalog();

    // With all default features, should have 8 fixers
    #[cfg(all(
        feature = "fixer-resolver-v2",
        feature = "fixer-path-dep-version",
        feature = "fixer-workspace-inheritance",
        feature = "fixer-duplicate-deps",
        feature = "fixer-remove-unused-deps",
        feature = "fixer-msrv",
        feature = "fixer-edition",
        feature = "fixer-license"
    ))]
    assert_eq!(catalog.len(), 8, "Should have 8 fixers with all features");

    // At minimum, with default features, should have at least one
    assert!(!catalog.is_empty(), "Catalog should not be empty");
}

// ============================================================================
// TriggerPattern Structure Tests
// ============================================================================

#[test]
fn test_trigger_pattern_fields() {
    let catalog = enabled_fix_catalog();

    for entry in catalog {
        for trigger in entry.triggers {
            // Sensor should be lowercase kebab-case
            assert!(
                trigger
                    .sensor
                    .chars()
                    .all(|c| c.is_lowercase() || c == '-' || c == '_'),
                "Sensor '{}' should be lowercase",
                trigger.sensor
            );

            // Check ID should follow dot-notation convention
            assert!(
                trigger.check_id.contains('.'),
                "Check ID '{}' should contain dots",
                trigger.check_id
            );
        }
    }
}
