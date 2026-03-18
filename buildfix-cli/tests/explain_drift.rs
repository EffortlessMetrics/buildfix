//! Integration tests to prevent explain/metadata drift.
//!
//! These tests verify that the explanation surfaces match implementation,
//! ensuring "explanation integrity" across the codebase.
//!
//! # What's Tested
//!
//! 1. **Explain Output Matches Fixer Registry**
//!    - Every fixer in the catalog has a corresponding explain entry
//!    - Every explain entry maps to an actual fixer
//!    - No orphaned explanations or undocumented fixers
//!
//! 2. **Fix Metadata Consistency**
//!    - Each fixer's metadata matches between catalog and explain
//!    - Safety classes are consistent across code and docs
//!    - Policy keys match fixer identifiers
//!
//! 3. **Trigger Pattern Alignment**
//!    - Trigger patterns in catalog match those in explain registry
//!    - All sensors and check_ids are properly documented

use std::collections::{HashMap, HashSet};

/// Helper to collect catalog entries into a map by fix_id.
fn catalog_by_fix_id() -> HashMap<&'static str, buildfix_fixer_catalog::FixerCatalogEntry> {
    buildfix_fixer_catalog::enabled_fix_catalog()
        .into_iter()
        .map(|entry| (entry.fix_id, entry))
        .collect()
}

/// Helper to collect explain entries into a map by fix_id.
fn explain_by_fix_id() -> HashMap<&'static str, &'static buildfix_cli::explain::FixExplanation> {
    buildfix_cli::explain::enabled_fixes()
        .into_iter()
        .map(|fix| (fix.fix_id, fix))
        .collect()
}

// =============================================================================
// 1. EXPLAIN OUTPUT MATCHES FIXER REGISTRY
// =============================================================================

mod registry_alignment {
    use super::*;

    /// Test that every fixer in the catalog has a corresponding explain entry.
    ///
    /// This ensures no fixer is "undocumented" in the explain surface.
    #[test]
    fn every_catalog_entry_has_explain_entry() {
        let catalog = catalog_by_fix_id();
        let explain = explain_by_fix_id();

        let mut missing: Vec<&str> = Vec::new();

        for fix_id in catalog.keys() {
            if !explain.contains_key(fix_id) {
                missing.push(fix_id);
            }
        }

        assert!(
            missing.is_empty(),
            "catalog entries missing from explain registry: {missing:?}\n\
             Add corresponding FixExplanation entries to FIX_REGISTRY in explain.rs"
        );
    }

    /// Test that every explain entry maps to an actual fixer in the catalog.
    ///
    /// This ensures no "orphaned" explanations exist for non-existent fixers.
    #[test]
    fn every_explain_entry_has_catalog_entry() {
        let catalog = catalog_by_fix_id();
        let explain = explain_by_fix_id();

        let mut orphaned: Vec<&str> = Vec::new();

        for fix_id in explain.keys() {
            if !catalog.contains_key(fix_id) {
                orphaned.push(fix_id);
            }
        }

        assert!(
            orphaned.is_empty(),
            "explain entries without catalog entries (orphaned): {orphaned:?}\n\
             Remove FixExplanation entries from FIX_REGISTRY or add fixer to catalog"
        );
    }

    /// Test that keys are unique across both registries.
    #[test]
    fn keys_are_unique_in_catalog() {
        let catalog = buildfix_fixer_catalog::enabled_fix_catalog();
        let keys: Vec<&str> = catalog.iter().map(|e| e.key).collect();
        let unique: HashSet<&str> = keys.iter().copied().collect();

        assert_eq!(keys.len(), unique.len(), "duplicate keys found in catalog");
    }

    #[test]
    fn keys_are_unique_in_explain() {
        let explain = buildfix_cli::explain::enabled_fixes();
        let keys: Vec<&str> = explain.iter().map(|f| f.key).collect();
        let unique: HashSet<&str> = keys.iter().copied().collect();

        assert_eq!(
            keys.len(),
            unique.len(),
            "duplicate keys found in explain registry"
        );
    }

    /// Test that fix_ids are unique across both registries.
    #[test]
    fn fix_ids_are_unique_in_catalog() {
        let catalog = buildfix_fixer_catalog::enabled_fix_catalog();
        let ids: Vec<&str> = catalog.iter().map(|e| e.fix_id).collect();
        let unique: HashSet<&str> = ids.iter().copied().collect();

        assert_eq!(
            ids.len(),
            unique.len(),
            "duplicate fix_ids found in catalog"
        );
    }

    #[test]
    fn fix_ids_are_unique_in_explain() {
        let explain = buildfix_cli::explain::enabled_fixes();
        let ids: Vec<&str> = explain.iter().map(|f| f.fix_id).collect();
        let unique: HashSet<&str> = ids.iter().copied().collect();

        assert_eq!(
            ids.len(),
            unique.len(),
            "duplicate fix_ids found in explain registry"
        );
    }
}

// =============================================================================
// 2. FIX METADATA CONSISTENCY
// =============================================================================

mod metadata_consistency {
    use super::*;
    use buildfix_types::ops::SafetyClass;

    /// Test that safety classes match between catalog and explain.
    ///
    /// Safety class is critical for user trust - if catalog says Safe
    /// but explain says Guarded, users will be confused.
    #[test]
    fn safety_classes_match() {
        let catalog = catalog_by_fix_id();
        let explain = explain_by_fix_id();

        let mut mismatches: Vec<(&str, SafetyClass, SafetyClass)> = Vec::new();

        for (fix_id, catalog_entry) in &catalog {
            if let Some(explain_entry) = explain.get(fix_id)
                && catalog_entry.safety != explain_entry.safety
            {
                mismatches.push((fix_id, catalog_entry.safety, explain_entry.safety));
            }
        }

        assert!(
            mismatches.is_empty(),
            "safety class mismatches between catalog and explain:\n\
             {:#?}\n\
             Ensure FixerCatalogEntry.safety matches FixExplanation.safety",
            mismatches
                .iter()
                .map(|(id, cat, exp)| format!("  {id}: catalog={:?} explain={:?}", cat, exp))
                .collect::<Vec<_>>()
        );
    }

    /// Test that keys match between catalog and explain for the same fix_id.
    #[test]
    fn keys_match() {
        let catalog = catalog_by_fix_id();
        let explain = explain_by_fix_id();

        let mut mismatches: Vec<(&str, &str, &str)> = Vec::new();

        for (fix_id, catalog_entry) in &catalog {
            if let Some(explain_entry) = explain.get(fix_id)
                && catalog_entry.key != explain_entry.key
            {
                mismatches.push((fix_id, catalog_entry.key, explain_entry.key));
            }
        }

        assert!(
            mismatches.is_empty(),
            "key mismatches between catalog and explain:\n\
             {:#?}\n\
             Ensure FixerCatalogEntry.key matches FixExplanation.key",
            mismatches
                .iter()
                .map(|(id, cat, exp)| format!("  {id}: catalog={} explain={}", cat, exp))
                .collect::<Vec<_>>()
        );
    }

    /// Test that lookup by key works for all catalog entries.
    #[test]
    fn lookup_by_key_works_for_all_catalog_entries() {
        let catalog = buildfix_fixer_catalog::enabled_fix_catalog();

        for entry in &catalog {
            let explain_entry = buildfix_cli::explain::lookup_fix(entry.key);
            assert!(
                explain_entry.is_some(),
                "lookup_fix(\"{}\") should return an entry for catalog fix_id={}",
                entry.key,
                entry.fix_id
            );

            let found = explain_entry.unwrap();
            assert_eq!(
                found.fix_id, entry.fix_id,
                "lookup_fix(\"{}\") returned wrong fix_id: expected {}, got {}",
                entry.key, entry.fix_id, found.fix_id
            );
        }
    }

    /// Test that lookup by fix_id works for all catalog entries.
    #[test]
    fn lookup_by_fix_id_works_for_all_catalog_entries() {
        let catalog = buildfix_fixer_catalog::enabled_fix_catalog();

        for entry in &catalog {
            let explain_entry = buildfix_cli::explain::lookup_fix(entry.fix_id);
            assert!(
                explain_entry.is_some(),
                "lookup_fix(\"{}\") should return an entry for key={}",
                entry.fix_id,
                entry.key
            );

            let found = explain_entry.unwrap();
            assert_eq!(
                found.fix_id, entry.fix_id,
                "lookup_fix(\"{}\") returned wrong fix_id: expected {}, got {}",
                entry.fix_id, entry.fix_id, found.fix_id
            );
        }
    }

    /// Test that list_fix_keys returns all catalog keys.
    #[test]
    fn list_fix_keys_matches_catalog() {
        let catalog_keys: HashSet<&str> = buildfix_fixer_catalog::enabled_fix_keys()
            .into_iter()
            .collect();
        let explain_keys: HashSet<&str> =
            buildfix_cli::explain::list_fix_keys().into_iter().collect();

        assert_eq!(
            catalog_keys, explain_keys,
            "list_fix_keys() should match catalog keys"
        );
    }

    /// Test that policy keys can be generated for all fixes.
    #[test]
    fn policy_keys_generated_for_all_fixes() {
        let fixes = buildfix_cli::explain::enabled_fixes();

        for fix in &fixes {
            let policy_keys = buildfix_cli::explain::policy_keys(fix);
            assert!(
                !policy_keys.is_empty(),
                "policy_keys should not be empty for fix {} ({})",
                fix.key,
                fix.fix_id
            );

            // Verify policy key format: sensor/check_id/code
            for key in &policy_keys {
                let parts: Vec<&str> = key.split('/').collect();
                assert_eq!(
                    parts.len(),
                    3,
                    "policy key '{}' should have format sensor/check_id/code",
                    key
                );
            }
        }
    }
}

// =============================================================================
// 3. TRIGGER PATTERN ALIGNMENT
// =============================================================================

mod trigger_alignment {
    use super::*;

    /// Helper to convert triggers to a comparable set of strings.
    fn triggers_to_set(triggers: &[buildfix_fixer_catalog::TriggerPattern]) -> HashSet<String> {
        triggers
            .iter()
            .map(|t| format!("{}/{}/{}", t.sensor, t.check_id, t.code.unwrap_or("*")))
            .collect()
    }

    /// Test that trigger patterns match between catalog and explain.
    ///
    /// This ensures that when a sensor finding triggers a fix,
    /// both the catalog and explain surfaces agree on what triggers exist.
    #[test]
    fn trigger_patterns_match() {
        let catalog = catalog_by_fix_id();
        let explain = explain_by_fix_id();

        let mut missing_in_explain: Vec<(&str, String)> = Vec::new();
        let mut extra_in_explain: Vec<(&str, String)> = Vec::new();

        for (fix_id, catalog_entry) in &catalog {
            if let Some(explain_entry) = explain.get(fix_id) {
                let catalog_triggers = triggers_to_set(catalog_entry.triggers);
                let explain_triggers = triggers_to_set(explain_entry.triggers);

                // Check for triggers in catalog but not in explain
                for trigger in catalog_triggers.difference(&explain_triggers) {
                    missing_in_explain.push((*fix_id, trigger.clone()));
                }

                // Check for triggers in explain but not in catalog
                for trigger in explain_triggers.difference(&catalog_triggers) {
                    extra_in_explain.push((*fix_id, trigger.clone()));
                }
            }
        }

        let mut errors = Vec::new();

        if !missing_in_explain.is_empty() {
            errors.push(format!(
                "triggers in catalog but missing from explain:\n{}",
                missing_in_explain
                    .iter()
                    .map(|(id, t)| format!("  {id}: {t}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }

        if !extra_in_explain.is_empty() {
            errors.push(format!(
                "triggers in explain but not in catalog:\n{}",
                extra_in_explain
                    .iter()
                    .map(|(id, t)| format!("  {id}: {t}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }

        assert!(
            errors.is_empty(),
            "trigger pattern mismatches:\n{}",
            errors.join("\n\n")
        );
    }

    /// Test that all triggers have non-empty sensor and check_id.
    #[test]
    fn triggers_have_valid_sensor_and_check_id() {
        let fixes = buildfix_cli::explain::enabled_fixes();

        for fix in &fixes {
            for trigger in fix.triggers {
                assert!(
                    !trigger.sensor.is_empty(),
                    "trigger sensor is empty for fix {} ({})",
                    fix.key,
                    fix.fix_id
                );
                assert!(
                    !trigger.check_id.is_empty(),
                    "trigger check_id is empty for fix {} ({})",
                    fix.key,
                    fix.fix_id
                );
            }
        }
    }

    /// Test that all catalog triggers have valid sensor and check_id.
    #[test]
    fn catalog_triggers_have_valid_sensor_and_check_id() {
        let catalog = buildfix_fixer_catalog::enabled_fix_catalog();

        for entry in &catalog {
            for trigger in entry.triggers {
                assert!(
                    !trigger.sensor.is_empty(),
                    "trigger sensor is empty for catalog entry {} ({})",
                    entry.key,
                    entry.fix_id
                );
                assert!(
                    !trigger.check_id.is_empty(),
                    "trigger check_id is empty for catalog entry {} ({})",
                    entry.key,
                    entry.fix_id
                );
            }
        }
    }
}

// =============================================================================
// 4. DOCUMENTATION CONTENT VALIDATION
// =============================================================================

mod documentation_validation {

    /// Test that all explain entries have non-empty required fields.
    #[test]
    fn explain_entries_have_required_content() {
        let fixes = buildfix_cli::explain::enabled_fixes();

        for fix in &fixes {
            assert!(
                !fix.key.is_empty(),
                "fix key is empty for fix_id={}",
                fix.fix_id
            );
            assert!(
                !fix.fix_id.is_empty(),
                "fix fix_id is empty for key={}",
                fix.key
            );
            assert!(
                !fix.title.is_empty(),
                "fix title is empty for key={}",
                fix.key
            );
            assert!(
                !fix.description.is_empty(),
                "fix description is empty for key={}",
                fix.key
            );
            assert!(
                !fix.safety_rationale.is_empty(),
                "fix safety_rationale is empty for key={}",
                fix.key
            );
            assert!(
                !fix.remediation.is_empty(),
                "fix remediation is empty for key={}",
                fix.key
            );
            assert!(
                !fix.triggers.is_empty(),
                "fix triggers is empty for key={}",
                fix.key
            );
        }
    }

    /// Test that description and safety_rationale are substantive.
    #[test]
    fn explain_entries_have_substantive_content() {
        let fixes = buildfix_cli::explain::enabled_fixes();

        for fix in &fixes {
            // Descriptions should be at least 50 characters (a sentence)
            assert!(
                fix.description.len() >= 50,
                "description too short ({}) for key={}, should be substantive",
                fix.description.len(),
                fix.key
            );

            // Safety rationale should be at least 50 characters
            assert!(
                fix.safety_rationale.len() >= 50,
                "safety_rationale too short ({}) for key={}, should be substantive",
                fix.safety_rationale.len(),
                fix.key
            );

            // Remediation should be at least 30 characters
            assert!(
                fix.remediation.len() >= 30,
                "remediation too short ({}) for key={}, should be substantive",
                fix.remediation.len(),
                fix.key
            );
        }
    }

    /// Test that titles follow a consistent naming convention.
    #[test]
    fn titles_follow_naming_convention() {
        let fixes = buildfix_cli::explain::enabled_fixes();

        for fix in &fixes {
            // Titles should not end with "Fix"
            assert!(
                !fix.title.ends_with(" Fix"),
                "title '{}' should not end with ' Fix' (redundant)",
                fix.title
            );

            // Titles should be title case (first letter of each word capitalized)
            let has_lowercase_start = fix.title.split_whitespace().any(|word| {
                word.chars()
                    .next()
                    .map(|c| c.is_lowercase() && c.is_alphabetic())
                    .unwrap_or(false)
            });

            // Allow some exceptions for technical terms like "v2"
            let is_exception = fix.key == "resolver-v2" || fix.key == "msrv";

            assert!(
                is_exception || !has_lowercase_start,
                "title '{}' should use title case",
                fix.title
            );
        }
    }
}

// =============================================================================
// 5. CROSS-CRATE CONSISTENCY
// =============================================================================

mod cross_crate_consistency {
    use super::*;

    /// Test that buildfix-core's builtin_fixer_metas matches the catalog.
    ///
    /// This ensures the domain layer and CLI layer agree on what fixers exist.
    #[test]
    fn core_metas_match_catalog() {
        let catalog_ids: HashSet<&str> = buildfix_fixer_catalog::enabled_fix_ids()
            .into_iter()
            .collect();

        let core_ids: HashSet<&str> = buildfix_core::builtin_fixer_metas()
            .into_iter()
            .map(|m| m.fix_key)
            .collect();

        assert_eq!(
            catalog_ids, core_ids,
            "catalog fix_ids should match buildfix_core::builtin_fixer_metas fix_key values"
        );
    }

    /// Test that enabled_fix_ids matches enabled_fix_catalog keys.
    #[test]
    fn enabled_fix_ids_matches_catalog() {
        let catalog_ids: HashSet<&str> = buildfix_fixer_catalog::enabled_fix_catalog()
            .iter()
            .map(|e| e.fix_id)
            .collect();

        let ids: HashSet<&str> = buildfix_fixer_catalog::enabled_fix_ids()
            .into_iter()
            .collect();

        assert_eq!(
            catalog_ids, ids,
            "enabled_fix_ids() should return all fix_ids from enabled_fix_catalog()"
        );
    }

    /// Test that enabled_fix_keys matches enabled_fix_catalog keys.
    #[test]
    fn enabled_fix_keys_matches_catalog() {
        let catalog_keys: HashSet<&str> = buildfix_fixer_catalog::enabled_fix_catalog()
            .iter()
            .map(|e| e.key)
            .collect();

        let keys: HashSet<&str> = buildfix_fixer_catalog::enabled_fix_keys()
            .into_iter()
            .collect();

        assert_eq!(
            catalog_keys, keys,
            "enabled_fix_keys() should return all keys from enabled_fix_catalog()"
        );
    }
}

// =============================================================================
// 6. LOOKUP ROBUSTNESS
// =============================================================================

mod lookup_robustness {

    /// Test case-insensitive lookup.
    #[test]
    fn lookup_is_case_insensitive() {
        let catalog = buildfix_fixer_catalog::enabled_fix_catalog();

        for entry in &catalog {
            let upper = entry.key.to_uppercase();
            let lower = entry.key.to_lowercase();

            let result_upper = buildfix_cli::explain::lookup_fix(&upper);
            let result_lower = buildfix_cli::explain::lookup_fix(&lower);

            assert!(
                result_upper.is_some(),
                "lookup_fix(\"{}\") should find entry for key={}",
                upper,
                entry.key
            );
            assert!(
                result_lower.is_some(),
                "lookup_fix(\"{}\") should find entry for key={}",
                lower,
                entry.key
            );
        }
    }

    /// Test underscore-to-hyphen normalization in lookup.
    #[test]
    fn lookup_handles_underscores() {
        let catalog = buildfix_fixer_catalog::enabled_fix_catalog();

        for entry in &catalog {
            if entry.key.contains('-') {
                let underscored = entry.key.replace('-', "_");

                let result = buildfix_cli::explain::lookup_fix(&underscored);
                assert!(
                    result.is_some(),
                    "lookup_fix(\"{}\") should find entry for key={}",
                    underscored,
                    entry.key
                );
            }
        }
    }

    /// Test lookup by fix_id suffix.
    #[test]
    fn lookup_by_fix_id_suffix() {
        let catalog = buildfix_fixer_catalog::enabled_fix_catalog();

        for entry in &catalog {
            // Extract suffix after the dot (e.g., "workspace_resolver_v2" from "cargo.workspace_resolver_v2")
            if let Some(suffix) = entry.fix_id.split('.').next_back() {
                let result = buildfix_cli::explain::lookup_fix(suffix);
                assert!(
                    result.is_some(),
                    "lookup_fix(\"{}\") should find entry for fix_id={}",
                    suffix,
                    entry.fix_id
                );
            }
        }
    }

    /// Test that lookup returns None for non-existent keys.
    #[test]
    fn lookup_returns_none_for_invalid_keys() {
        let invalid_keys = [
            "nonexistent-fix",
            "fake_fix",
            "cargo.nonexistent",
            "random-string-12345",
        ];

        for key in &invalid_keys {
            let result = buildfix_cli::explain::lookup_fix(key);
            assert!(
                result.is_none(),
                "lookup_fix(\"{}\") should return None for non-existent key",
                key
            );
        }
    }
}
