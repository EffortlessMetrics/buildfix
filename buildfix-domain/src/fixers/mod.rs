use buildfix_fixer_api::{Fixer, FixerMeta};

#[cfg(feature = "fixer-duplicate-deps")]
use buildfix_fixer_duplicate_deps as duplicate_deps;
#[cfg(feature = "fixer-edition")]
use buildfix_fixer_edition as edition;
#[cfg(feature = "fixer-license")]
use buildfix_fixer_license as license;
#[cfg(feature = "fixer-msrv")]
use buildfix_fixer_msrv as msrv;
#[cfg(feature = "fixer-path-dep-version")]
use buildfix_fixer_path_dep_version as path_dep_version;
#[cfg(feature = "fixer-remove-unused-deps")]
use buildfix_fixer_remove_unused_deps as remove_unused_deps;
#[cfg(feature = "fixer-resolver-v2")]
use buildfix_fixer_resolver_v2 as resolver_v2;
#[cfg(feature = "fixer-workspace-inheritance")]
use buildfix_fixer_workspace_inheritance as workspace_inheritance;

#[allow(clippy::vec_init_then_push)]
pub fn builtin_fixers() -> Vec<Box<dyn Fixer>> {
    let mut fixers: Vec<Box<dyn Fixer>> = Vec::new();

    #[cfg(feature = "fixer-resolver-v2")]
    fixers.push(Box::new(resolver_v2::ResolverV2Fixer));
    #[cfg(feature = "fixer-path-dep-version")]
    fixers.push(Box::new(path_dep_version::PathDepVersionFixer));
    #[cfg(feature = "fixer-workspace-inheritance")]
    fixers.push(Box::new(workspace_inheritance::WorkspaceInheritanceFixer));
    #[cfg(feature = "fixer-duplicate-deps")]
    fixers.push(Box::new(duplicate_deps::DuplicateDepsConsolidationFixer));
    #[cfg(feature = "fixer-remove-unused-deps")]
    fixers.push(Box::new(remove_unused_deps::RemoveUnusedDepsFixer));
    #[cfg(feature = "fixer-msrv")]
    fixers.push(Box::new(msrv::MsrvNormalizeFixer));
    #[cfg(feature = "fixer-edition")]
    fixers.push(Box::new(edition::EditionUpgradeFixer));
    #[cfg(feature = "fixer-license")]
    fixers.push(Box::new(license::LicenseNormalizeFixer));

    fixers
}

#[cfg(test)]
fn expected_fixer_ids() -> Vec<&'static str> {
    buildfix_fixer_catalog::enabled_fix_ids()
}

/// Returns metadata for all builtin fixers.
pub fn builtin_fixer_metas() -> Vec<FixerMeta> {
    builtin_fixers().iter().map(|f| f.meta()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn builtin_fixers_have_unique_keys() {
        let fixers = builtin_fixers();
        let expected = expected_fixer_ids();

        assert_eq!(fixers.len(), expected.len());

        let mut keys = BTreeSet::new();
        for fixer in fixers {
            let meta = fixer.meta();
            assert!(!meta.fix_key.is_empty());
            assert!(!meta.description.is_empty());
            keys.insert(meta.fix_key);
        }

        let expected: BTreeSet<&'static str> = expected.into_iter().collect();
        assert_eq!(keys, expected);
    }

    #[test]
    fn builtin_fixer_metas_matches_fixers() {
        let metas = builtin_fixer_metas();
        let keys: BTreeSet<&'static str> = metas.iter().map(|m| m.fix_key).collect();
        let expected = expected_fixer_ids()
            .into_iter()
            .collect::<BTreeSet<&'static str>>();
        assert_eq!(keys, expected);
    }

    #[test]
    fn builtin_fixer_metas_align_with_catalog() {
        let mut metas = builtin_fixer_metas();
        let mut catalog = buildfix_fixer_catalog::enabled_fix_catalog();

        metas.sort_by_key(|m| m.fix_key);
        catalog.sort_by_key(|entry| entry.fix_id);

        assert_eq!(metas.len(), catalog.len(), "enabled fixer count mismatch");

        for (meta, entry) in metas.iter().zip(catalog.iter()) {
            assert_eq!(
                meta.fix_key, entry.fix_id,
                "fix key mismatch for entry {}",
                meta.fix_key
            );
            assert_eq!(
                meta.safety, entry.safety,
                "safety mismatch for {}",
                meta.fix_key
            );

            let meta_sensors: BTreeSet<&'static str> =
                meta.consumes_sensors.iter().copied().collect();
            let entry_sensors: BTreeSet<&'static str> =
                entry.triggers.iter().map(|t| t.sensor).collect();

            let meta_check_ids: BTreeSet<&'static str> =
                meta.consumes_check_ids.iter().copied().collect();
            let entry_check_ids: BTreeSet<&'static str> =
                entry.triggers.iter().map(|t| t.check_id).collect();

            assert_eq!(
                meta_sensors, entry_sensors,
                "sensor mismatch for {}",
                meta.fix_key
            );
            assert_eq!(
                meta_check_ids, entry_check_ids,
                "check_id mismatch for {}",
                meta.fix_key
            );
        }
    }
}
