//! Edit engine for buildfix plans.
//!
//! Responsibilities:
//! - Attach file preconditions (sha256) to a plan.
//! - Apply operations (in-memory or to disk) using `toml_edit`.
//! - Generate a unified diff preview.

use anyhow::Context;
use buildfix_types::apply::{
    ApplyStatus, ApplySummary, AppliedFixResult, BuildfixApply, FileChange, PreconditionResult,
};
use buildfix_types::ops::{Operation, SafetyClass};
use buildfix_types::plan::{BuildfixPlan, Precondition};
use buildfix_types::receipt::ToolInfo;
use camino::{Utf8Path, Utf8PathBuf};
use chrono::Utc;
use diffy::PatchFormatter;
use fs_err as fs;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use toml_edit::{value, DocumentMut, InlineTable, Item};

#[derive(Debug, Clone)]
pub struct ApplyOptions {
    pub dry_run: bool,
    pub allow_guarded: bool,
    pub allow_unsafe: bool,
}

impl Default for ApplyOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            allow_guarded: false,
            allow_unsafe: false,
        }
    }
}

/// Attach per-fix preconditions (FileExists + FileSha256) for each file touched by operations.
pub fn attach_preconditions(repo_root: &Utf8Path, plan: &mut BuildfixPlan) -> anyhow::Result<()> {
    for fix in plan.fixes.iter_mut() {
        let mut files = BTreeSet::new();
        for op in &fix.operations {
            files.insert(op.manifest().clone());
        }

        let mut pres = Vec::new();
        for path in files {
            let abs = abs_path(repo_root, &path);

            pres.push(Precondition::FileExists { path: path.clone() });

            let bytes = fs::read(&abs).with_context(|| format!("read {}", abs))?;
            let sha = sha256_hex(&bytes);
            pres.push(Precondition::FileSha256 {
                path: path.clone(),
                sha256: sha,
            });
        }
        fix.preconditions = pres;
    }

    Ok(())
}

fn abs_path(repo_root: &Utf8Path, rel: &Utf8Path) -> Utf8PathBuf {
    if rel.is_absolute() {
        rel.to_path_buf()
    } else {
        repo_root.join(rel)
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn glob_match(pat: &str, text: &str) -> bool {
    // Simple wildcard matcher: '*' and '?'.
    //
    // DP implementation to avoid recursion.
    let p = pat.as_bytes();
    let t = text.as_bytes();
    let mut dp = vec![vec![false; t.len() + 1]; p.len() + 1];
    dp[0][0] = true;

    for i in 1..=p.len() {
        if p[i - 1] == b'*' {
            dp[i][0] = dp[i - 1][0];
        }
    }

    for i in 1..=p.len() {
        for j in 1..=t.len() {
            dp[i][j] = match p[i - 1] {
                b'*' => dp[i - 1][j] || dp[i][j - 1],
                b'?' => dp[i - 1][j - 1],
                c => dp[i - 1][j - 1] && c == t[j - 1],
            };
        }
    }

    dp[p.len()][t.len()]
}

fn allowed_by_id(plan: &BuildfixPlan, fix_id: &str) -> bool {
    if !plan.policy.allow.is_empty() {
        let any_allow = plan.policy.allow.iter().any(|p| glob_match(p, fix_id));
        if !any_allow {
            return false;
        }
    }

    if plan.policy.deny.iter().any(|p| glob_match(p, fix_id)) {
        return false;
    }

    true
}

fn allowed_by_safety(opts: &ApplyOptions, safety: SafetyClass) -> bool {
    match safety {
        SafetyClass::Safe => true,
        SafetyClass::Guarded => opts.allow_guarded,
        SafetyClass::Unsafe => opts.allow_unsafe,
    }
}

pub fn preview_patch(
    repo_root: &Utf8Path,
    plan: &BuildfixPlan,
    opts: &ApplyOptions,
) -> anyhow::Result<String> {
    let outcome = execute_plan(repo_root, plan, opts)?;
    Ok(render_patch(&outcome.before, &outcome.after))
}

/// Apply a plan. When `opts.dry_run` is true, no files are written, but results and a patch are still produced.
pub fn apply_plan(
    repo_root: &Utf8Path,
    plan: &BuildfixPlan,
    tool: ToolInfo,
    opts: &ApplyOptions,
) -> anyhow::Result<(BuildfixApply, String)> {
    let outcome = execute_plan(repo_root, plan, opts)?;
    let patch = render_patch(&outcome.before, &outcome.after);

    if !opts.dry_run {
        // Write only changed files.
        for (path, new_contents) in &outcome.after {
            let old = outcome.before.get(path).cloned().unwrap_or_default();
            if &old == new_contents {
                continue;
            }
            let abs = abs_path(repo_root, path);
            fs::write(&abs, new_contents).with_context(|| format!("write {}", abs))?;
        }
    }

    let mut apply = BuildfixApply::new(tool, plan.plan_id.clone());
    apply.applied = !opts.dry_run;
    apply.summary = outcome.summary;
    apply.results = outcome.results;
    apply.run.ended_at = Some(Utc::now());

    Ok((apply, patch))
}

struct ExecuteOutcome {
    before: BTreeMap<Utf8PathBuf, String>,
    after: BTreeMap<Utf8PathBuf, String>,
    results: Vec<AppliedFixResult>,
    summary: ApplySummary,
}

fn execute_plan(
    repo_root: &Utf8Path,
    plan: &BuildfixPlan,
    opts: &ApplyOptions,
) -> anyhow::Result<ExecuteOutcome> {
    // Read initial state for all touched files.
    let mut touched_files = BTreeSet::new();
    for fix in &plan.fixes {
        for op in &fix.operations {
            touched_files.insert(op.manifest().clone());
        }
    }

    let mut before: BTreeMap<Utf8PathBuf, String> = BTreeMap::new();
    for p in &touched_files {
        let abs = abs_path(repo_root, p);
        let contents = fs::read_to_string(&abs).unwrap_or_default();
        before.insert(p.clone(), contents);
    }

    let mut current = before.clone();

    let mut results: Vec<AppliedFixResult> = Vec::new();
    let mut summary = ApplySummary::default();

    for fix in &plan.fixes {
        let mut result = AppliedFixResult {
            fix_id: fix.fix_id.clone(),
            fix_instance_id: fix.id.clone(),
            safety: fix.safety,
            title: fix.title.clone(),
            preconditions: vec![],
            status: ApplyStatus::Skipped,
            message: None,
            files_changed: vec![],
        };

        if !allowed_by_id(plan, &fix.fix_id.0) {
            result.status = ApplyStatus::Skipped;
            result.message = Some("skipped: denied by policy".to_string());
            summary.skipped += 1;
            results.push(result);
            continue;
        }

        if !allowed_by_safety(opts, fix.safety) {
            result.status = ApplyStatus::Skipped;
            result.message = Some("skipped: safety class not allowed".to_string());
            summary.skipped += 1;
            results.push(result);
            continue;
        }

        // Preconditions
        let mut pre_ok = true;
        for pre in &fix.preconditions {
            let (ok, msg) = eval_precondition(repo_root, pre)?;
            if !ok {
                pre_ok = false;
            }
            result.preconditions.push(PreconditionResult {
                precondition: pre.clone(),
                ok,
                message: msg,
            });
        }

        if plan.policy.require_clean_hashes && !pre_ok {
            result.status = ApplyStatus::Failed;
            result.message = Some("precondition failed".to_string());
            summary.failed += 1;
            results.push(result);
            continue;
        }

        summary.attempted += 1;

        let touched = files_touched(fix);
        let mut before_snap: BTreeMap<Utf8PathBuf, String> = BTreeMap::new();
        for p in &touched {
            before_snap.insert(p.clone(), current.get(p).cloned().unwrap_or_default());
        }

        // Apply operations sequentially to current map.
        for op in &fix.operations {
            let file = op.manifest().clone();
            let old = current.get(&file).cloned().unwrap_or_default();
            let new = apply_op_to_content(&old, op)
                .with_context(|| format!("apply op {:?} to {}", op, file))?;
            current.insert(file, new);
        }

        // Record changes for this fix.
        for p in &touched {
            let before_c = before_snap.get(p).cloned().unwrap_or_default();
            let after_c = current.get(p).cloned().unwrap_or_default();
            if before_c != after_c {
                result.files_changed.push(file_change(p, &before_c, &after_c));
            }
        }

        if opts.dry_run {
            result.status = ApplyStatus::Skipped;
            result.message = Some("dry-run: not written".to_string());
            summary.skipped += 1;
        } else {
            result.status = ApplyStatus::Applied;
            summary.applied += 1;
        }

        results.push(result);
    }

    Ok(ExecuteOutcome {
        before,
        after: current,
        results,
        summary,
    })
}

fn file_change(path: &Utf8PathBuf, before: &str, after: &str) -> FileChange {
    let before_bytes = before.as_bytes();
    let after_bytes = after.as_bytes();
    FileChange {
        path: path.to_string(),
        before_sha256: sha256_hex(before_bytes),
        after_sha256: sha256_hex(after_bytes),
        before_bytes: Some(before_bytes.len() as u64),
        after_bytes: Some(after_bytes.len() as u64),
        applied_at: Some(Utc::now()),
    }
}

fn files_touched(fix: &buildfix_types::plan::PlannedFix) -> BTreeSet<Utf8PathBuf> {
    let mut set = BTreeSet::new();
    for op in &fix.operations {
        set.insert(op.manifest().clone());
    }
    set
}

fn eval_precondition(repo_root: &Utf8Path, pre: &Precondition) -> anyhow::Result<(bool, Option<String>)> {
    match pre {
        Precondition::FileExists { path } => {
            let abs = abs_path(repo_root, path);
            let ok = abs.exists();
            Ok((ok, if ok { None } else { Some("file missing".to_string()) }))
        }
        Precondition::FileSha256 { path, sha256 } => {
            let abs = abs_path(repo_root, path);
            let bytes = fs::read(&abs).with_context(|| format!("read {}", abs))?;
            let actual = sha256_hex(&bytes);
            let ok = &actual == sha256;
            Ok((
                ok,
                if ok {
                    None
                } else {
                    Some(format!("sha mismatch: expected {sha256}, got {actual}"))
                },
            ))
        }
    }
}

fn render_patch(
    before: &BTreeMap<Utf8PathBuf, String>,
    after: &BTreeMap<Utf8PathBuf, String>,
) -> String {
    let mut out = String::new();
    let mut formatter = PatchFormatter::new();

    for (path, old) in before {
        let new = after.get(path).unwrap_or(old);
        if old == new {
            continue;
        }

        out.push_str(&format!("diff --git a/{0} b/{0}\n", path));
        out.push_str(&format!("--- a/{0}\n+++ b/{0}\n", path));

        let patch = diffy::create_patch(old, new);
        out.push_str(&formatter.fmt_patch(&patch));
        if !out.ends_with('\n') {
            out.push('\n');
        }
    }

    out
}

fn apply_op_to_content(contents: &str, op: &Operation) -> anyhow::Result<String> {
    let mut doc = contents.parse::<DocumentMut>().unwrap_or_else(|_| DocumentMut::new());

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
            let dep_item =
                get_dep_item_mut(&mut doc, toml_path).context("dependency not found at toml_path")?;

            if let Some(inline) = dep_item.as_inline_table_mut() {
                let current_path = inline.get("path").and_then(|v| v.as_str());
                if current_path != Some(dep_path.as_str()) {
                    return Ok(doc.to_string());
                }
                if inline.get("version").and_then(|v| v.as_str()).is_none() {
                    inline.insert("version", str_value(version));
                }
            } else if let Some(tbl) = dep_item.as_table_mut() {
                let current_path = tbl
                    .get("path")
                    .and_then(|i| i.as_value())
                    .and_then(|v| v.as_str());
                if current_path != Some(dep_path.as_str()) {
                    return Ok(doc.to_string());
                }
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

        Operation::UseWorkspaceDependency {
            toml_path,
            preserved,
            ..
        } => {
            let dep_item =
                get_dep_item_mut(&mut doc, toml_path).context("dependency not found at toml_path")?;

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

    Ok(doc.to_string())
}

fn str_value(s: &str) -> toml_edit::Value {
    value(s).as_value().unwrap().clone()
}

fn bool_value(b: bool) -> toml_edit::Value {
    value(b).as_value().unwrap().clone()
}

fn get_dep_item_mut<'a>(doc: &'a mut DocumentMut, toml_path: &[String]) -> Option<&'a mut Item> {
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
