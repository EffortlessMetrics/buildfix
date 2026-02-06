//! Edit engine for buildfix plans.
//!
//! Responsibilities:
//! - Attach file preconditions (sha256) to a plan.
//! - Apply operations (in-memory or to disk) using `toml_edit`.
//! - Generate a unified diff preview.

mod error;

pub use error::{EditError, EditResult, PolicyBlockError};

use anyhow::Context;
use buildfix_types::apply::{
    ApplyFile, ApplyPreconditions, ApplyRepoInfo, ApplyResult, ApplyStatus, ApplySummary,
    BuildfixApply, PlanRef, PreconditionMismatch,
};
use buildfix_types::ops::{OpKind, SafetyClass};
use buildfix_types::plan::{BuildfixPlan, FilePrecondition, PlanOp};
use buildfix_types::receipt::ToolInfo;
use camino::{Utf8Path, Utf8PathBuf};
use diffy::PatchFormatter;
use fs_err as fs;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use toml_edit::{DocumentMut, InlineTable, Item, value};
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct ApplyOptions {
    pub dry_run: bool,
    pub allow_guarded: bool,
    pub allow_unsafe: bool,
    pub backup_enabled: bool,
    /// Directory to store backups.
    pub backup_dir: Option<Utf8PathBuf>,
    /// Backup file suffix.
    pub backup_suffix: String,
    /// Params to resolve unsafe operations.
    pub params: HashMap<String, String>,
}

/// Options for attaching preconditions to a plan.
#[derive(Debug, Clone, Default)]
pub struct AttachPreconditionsOptions {
    /// If true, attach a `head_sha` precondition requiring the repo HEAD to match.
    pub include_git_head: bool,
}

/// Get the current git HEAD SHA for a repository.
pub fn get_head_sha(repo_root: &Utf8Path) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(repo_root)
        .output()
        .context("failed to run git rev-parse HEAD")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git rev-parse HEAD failed: {}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Check if the git working tree has uncommitted changes.
pub fn is_working_tree_dirty(repo_root: &Utf8Path) -> anyhow::Result<bool> {
    let status_output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root)
        .output()
        .context("failed to run git status")?;

    if !status_output.status.success() {
        let stderr = String::from_utf8_lossy(&status_output.stderr);
        anyhow::bail!("git status failed: {}", stderr.trim());
    }

    Ok(!status_output.stdout.is_empty())
}

/// Attach plan-level preconditions (FileSha256) for each file touched by ops.
///
/// Optionally attaches a git HEAD SHA precondition.
pub fn attach_preconditions(
    repo_root: &Utf8Path,
    plan: &mut BuildfixPlan,
    opts: &AttachPreconditionsOptions,
) -> anyhow::Result<()> {
    let mut files = BTreeSet::new();
    for op in &plan.ops {
        files.insert(op.target.path.clone());
    }

    let mut pres = Vec::new();
    for path in files {
        let abs = abs_path(repo_root, Utf8Path::new(&path));
        let bytes = fs::read(&abs).with_context(|| format!("read {}", abs))?;
        let sha = sha256_hex(&bytes);
        pres.push(FilePrecondition { path, sha256: sha });
    }
    plan.preconditions.files = pres;

    if opts.include_git_head
        && let Ok(sha) = get_head_sha(repo_root)
    {
        plan.preconditions.head_sha = Some(sha);
    }

    if let Ok(dirty) = is_working_tree_dirty(repo_root) {
        plan.preconditions.dirty = Some(dirty);
    }

    Ok(())
}

pub fn preview_patch(
    repo_root: &Utf8Path,
    plan: &BuildfixPlan,
    opts: &ApplyOptions,
) -> anyhow::Result<String> {
    let outcome = execute_plan(repo_root, plan, opts, false)?;
    Ok(render_patch(&outcome.before, &outcome.after))
}

/// Apply a plan. When `opts.dry_run` is true, no files are written, but results and a patch are still produced.
pub fn apply_plan(
    repo_root: &Utf8Path,
    plan: &BuildfixPlan,
    tool: ToolInfo,
    opts: &ApplyOptions,
) -> anyhow::Result<(BuildfixApply, String)> {
    let mut outcome = execute_plan(repo_root, plan, opts, true)?;
    let patch = render_patch(&outcome.before, &outcome.after);

    if !opts.dry_run && outcome.preconditions.verified {
        let changed_files = changed_files(&outcome.before, &outcome.after);
        if !changed_files.is_empty() {
            if opts.backup_enabled {
                create_backups(
                    repo_root,
                    &changed_files,
                    &outcome.before,
                    opts,
                    &mut outcome.results,
                )?;
            }
            write_changed_files(repo_root, &changed_files, &outcome.after)?;
        }
    }

    let repo_info = ApplyRepoInfo {
        root: repo_root.to_string(),
        head_sha_before: None,
        head_sha_after: None,
        dirty_before: None,
        dirty_after: None,
    };

    let plan_ref = PlanRef {
        path: "artifacts/buildfix/plan.json".to_string(),
        sha256: None,
    };

    let mut apply = BuildfixApply::new(tool, repo_info, plan_ref);
    apply.preconditions = outcome.preconditions;
    apply.results = outcome.results;
    apply.summary = outcome.summary;

    Ok((apply, patch))
}

struct ExecuteOutcome {
    before: BTreeMap<Utf8PathBuf, String>,
    after: BTreeMap<Utf8PathBuf, String>,
    results: Vec<ApplyResult>,
    summary: ApplySummary,
    preconditions: ApplyPreconditions,
}

fn execute_plan(
    repo_root: &Utf8Path,
    plan: &BuildfixPlan,
    opts: &ApplyOptions,
    verify_preconditions: bool,
) -> anyhow::Result<ExecuteOutcome> {
    let mut touched_files = BTreeSet::new();
    let mut resolved_ops: Vec<ResolvedOp> = Vec::new();

    for op in &plan.ops {
        let resolved = resolve_op(op, opts);
        if resolved.allowed {
            touched_files.insert(Utf8PathBuf::from(&op.target.path));
        }
        resolved_ops.push(resolved);
    }

    let mut before: BTreeMap<Utf8PathBuf, String> = BTreeMap::new();
    for p in &touched_files {
        let abs = abs_path(repo_root, p);
        let contents = fs::read_to_string(&abs).unwrap_or_default();
        before.insert(p.clone(), contents);
    }

    let mut preconditions = ApplyPreconditions {
        verified: true,
        mismatches: vec![],
    };

    if verify_preconditions
        && !check_preconditions(repo_root, plan, &touched_files, &mut preconditions)?
    {
        // Abort entire apply if any mismatch.
        let mut results = Vec::new();
        let mut summary = ApplySummary::default();

        for resolved in &resolved_ops {
            if !resolved.allowed {
                continue;
            }
            summary.blocked += 1;
            results.push(ApplyResult {
                op_id: resolved.op.id.clone(),
                status: ApplyStatus::Blocked,
                message: Some("precondition mismatch".to_string()),
                blocked_reason: Some("precondition mismatch".to_string()),
                files: vec![],
            });
        }

        return Ok(ExecuteOutcome {
            before: before.clone(),
            after: before,
            results,
            summary,
            preconditions,
        });
    }

    let mut current = before.clone();
    let mut results: Vec<ApplyResult> = Vec::new();
    let mut summary = ApplySummary::default();

    for resolved in &resolved_ops {
        let op = resolved.op;

        if !resolved.allowed {
            let mut res = ApplyResult {
                op_id: op.id.clone(),
                status: ApplyStatus::Blocked,
                message: None,
                blocked_reason: resolved.blocked_reason.clone(),
                files: vec![],
            };
            if let Some(msg) = &resolved.blocked_message {
                res.message = Some(msg.clone());
            }
            summary.blocked += 1;
            results.push(res);
            continue;
        }

        summary.attempted += 1;

        let file = Utf8PathBuf::from(&op.target.path);
        let old = current.get(&file).cloned().unwrap_or_default();

        let new = apply_op_to_content(&old, &resolved.kind)
            .with_context(|| format!("apply op {} to {}", op.id, op.target.path))?;

        current.insert(file.clone(), new.clone());

        let mut files = Vec::new();
        if old != new {
            files.push(ApplyFile {
                path: op.target.path.clone(),
                sha256_before: Some(sha256_hex(old.as_bytes())),
                sha256_after: Some(sha256_hex(new.as_bytes())),
                backup_path: None,
            });
        }

        if opts.dry_run {
            results.push(ApplyResult {
                op_id: op.id.clone(),
                status: ApplyStatus::Skipped,
                message: Some("dry-run: not written".to_string()),
                blocked_reason: None,
                files,
            });
        } else {
            summary.applied += 1;
            results.push(ApplyResult {
                op_id: op.id.clone(),
                status: ApplyStatus::Applied,
                message: None,
                blocked_reason: None,
                files,
            });
        }
    }

    summary.files_modified = changed_files(&before, &current).len() as u64;

    Ok(ExecuteOutcome {
        before,
        after: current,
        results,
        summary,
        preconditions,
    })
}

struct ResolvedOp<'a> {
    op: &'a PlanOp,
    kind: OpKind,
    allowed: bool,
    blocked_reason: Option<String>,
    blocked_message: Option<String>,
}

fn resolve_op<'a>(op: &'a PlanOp, opts: &ApplyOptions) -> ResolvedOp<'a> {
    if op.blocked {
        if !op.params_required.is_empty() {
            let (kind, missing) = resolve_params(op, &opts.params);
            if missing.is_empty() {
                return ResolvedOp {
                    op,
                    kind,
                    allowed: allowed_by_safety(opts, op.safety),
                    blocked_reason: None,
                    blocked_message: None,
                };
            }
            let blocked_reason = op
                .blocked_reason
                .clone()
                .or(Some("missing params".to_string()));
            return ResolvedOp {
                op,
                kind: op.kind.clone(),
                allowed: false,
                blocked_reason,
                blocked_message: None,
            };
        }

        let blocked_reason = op.blocked_reason.clone().or(Some("blocked".to_string()));
        return ResolvedOp {
            op,
            kind: op.kind.clone(),
            allowed: false,
            blocked_reason,
            blocked_message: None,
        };
    }

    if !allowed_by_safety(opts, op.safety) {
        return ResolvedOp {
            op,
            kind: op.kind.clone(),
            allowed: false,
            blocked_reason: Some("safety gate".to_string()),
            blocked_message: Some("safety class not allowed".to_string()),
        };
    }

    let (kind, missing) = resolve_params(op, &opts.params);
    if !missing.is_empty() {
        return ResolvedOp {
            op,
            kind,
            allowed: false,
            blocked_reason: Some(format!("missing params: {}", missing.join(", "))),
            blocked_message: None,
        };
    }

    ResolvedOp {
        op,
        kind,
        allowed: true,
        blocked_reason: None,
        blocked_message: None,
    }
}

fn resolve_params(op: &PlanOp, params: &HashMap<String, String>) -> (OpKind, Vec<String>) {
    if op.params_required.is_empty() {
        return (op.kind.clone(), Vec::new());
    }

    let mut missing = Vec::new();
    let mut kind = op.kind.clone();

    for key in &op.params_required {
        if let Some(value) = params.get(key) {
            fill_op_param(&mut kind, key, value);
        } else {
            missing.push(key.clone());
        }
    }

    (kind, missing)
}

fn fill_op_param(kind: &mut OpKind, key: &str, value: &str) {
    let OpKind::TomlTransform { rule_id, args } = kind else {
        return;
    };

    let mut map = match args.take() {
        Some(serde_json::Value::Object(m)) => m,
        _ => serde_json::Map::new(),
    };

    match (rule_id.as_str(), key) {
        ("set_package_rust_version", "rust_version") => {
            map.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
        ("ensure_path_dep_has_version", "version") => {
            map.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
        _ => {
            map.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }

    *args = Some(serde_json::Value::Object(map));
}

fn check_preconditions(
    repo_root: &Utf8Path,
    plan: &BuildfixPlan,
    touched_files: &BTreeSet<Utf8PathBuf>,
    preconditions: &mut ApplyPreconditions,
) -> anyhow::Result<bool> {
    let file_map = plan
        .preconditions
        .files
        .iter()
        .map(|f| (f.path.clone(), f.sha256.clone()))
        .collect::<BTreeMap<_, _>>();

    for file in touched_files {
        let Some(expected) = file_map.get(&file.to_string()) else {
            continue;
        };
        let abs = abs_path(repo_root, file);
        let bytes = fs::read(&abs).with_context(|| format!("read {}", abs))?;
        let actual = sha256_hex(&bytes);
        if &actual != expected {
            preconditions.verified = false;
            preconditions.mismatches.push(PreconditionMismatch {
                path: file.to_string(),
                expected: expected.clone(),
                actual,
            });
        }
    }

    if let Some(expected) = &plan.preconditions.head_sha
        && let Ok(actual) = get_head_sha(repo_root)
        && &actual != expected
    {
        preconditions.verified = false;
        preconditions.mismatches.push(PreconditionMismatch {
            path: "<git_head>".to_string(),
            expected: expected.clone(),
            actual,
        });
    }

    Ok(preconditions.verified)
}

fn changed_files(
    before: &BTreeMap<Utf8PathBuf, String>,
    after: &BTreeMap<Utf8PathBuf, String>,
) -> BTreeSet<Utf8PathBuf> {
    let mut changed = BTreeSet::new();
    for (path, old) in before {
        let new = after.get(path).unwrap_or(old);
        if old != new {
            changed.insert(path.clone());
        }
    }
    changed
}

fn create_backups(
    _repo_root: &Utf8Path,
    changed_files: &BTreeSet<Utf8PathBuf>,
    before: &BTreeMap<Utf8PathBuf, String>,
    opts: &ApplyOptions,
    results: &mut [ApplyResult],
) -> anyhow::Result<()> {
    let Some(ref backup_dir) = opts.backup_dir else {
        return Ok(());
    };

    for path in changed_files {
        let contents = before.get(path).cloned().unwrap_or_default();
        let backup_rel = format!("{}{}", path, opts.backup_suffix);
        let backup_path = backup_dir.join(backup_rel);

        if let Some(parent) = backup_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create backup dir {}", parent))?;
        }

        fs::write(&backup_path, &contents)
            .with_context(|| format!("write backup {}", backup_path))?;

        // Update any result entries that mention this file.
        for result in results.iter_mut() {
            for file in &mut result.files {
                if file.path == *path {
                    file.backup_path = Some(backup_path.to_string());
                }
            }
        }
    }

    Ok(())
}

fn write_changed_files(
    repo_root: &Utf8Path,
    changed_files: &BTreeSet<Utf8PathBuf>,
    after: &BTreeMap<Utf8PathBuf, String>,
) -> anyhow::Result<()> {
    for path in changed_files {
        let abs = abs_path(repo_root, path);
        let new_contents = after.get(path).cloned().unwrap_or_default();
        write_atomic(&abs, &new_contents)?;
    }
    Ok(())
}

fn write_atomic(path: &Utf8Path, contents: &str) -> anyhow::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    let tmp_name = format!(
        ".buildfix-tmp-{}",
        Uuid::new_v4().to_string().replace('-', "")
    );
    let tmp_path = parent.join(tmp_name);
    fs::write(&tmp_path, contents).with_context(|| format!("write {}", tmp_path))?;
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(&tmp_path, path).with_context(|| format!("rename {} -> {}", tmp_path, path))?;
    Ok(())
}

fn allowed_by_safety(opts: &ApplyOptions, safety: SafetyClass) -> bool {
    match safety {
        SafetyClass::Safe => true,
        SafetyClass::Guarded => opts.allow_guarded,
        SafetyClass::Unsafe => opts.allow_unsafe,
    }
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

fn render_patch(
    before: &BTreeMap<Utf8PathBuf, String>,
    after: &BTreeMap<Utf8PathBuf, String>,
) -> String {
    let mut out = String::new();
    let formatter = PatchFormatter::new();

    for (path, old) in before {
        let new = after.get(path).unwrap_or(old);
        if old == new {
            continue;
        }

        out.push_str(&format!("diff --git a/{0} b/{0}\n", path));
        out.push_str(&format!("--- a/{0}\n+++ b/{0}\n", path));

        let patch = diffy::create_patch(old, new);
        out.push_str(&formatter.fmt_patch(&patch).to_string());
        if !out.ends_with('\n') {
            out.push('\n');
        }
    }

    out
}

/// Applies a single operation to TOML content, returning the modified content.
///
/// This is the stable public API for pure TOML transforms. It parses the input
/// TOML, applies the [`OpKind`] transformation, and returns the modified string
/// preserving formatting.
pub fn apply_op_to_content(contents: &str, kind: &OpKind) -> anyhow::Result<String> {
    let mut doc = contents
        .parse::<DocumentMut>()
        .unwrap_or_else(|_| DocumentMut::new());

    match kind {
        OpKind::TomlSet { toml_path, value } => {
            set_toml_path(&mut doc, toml_path, value.clone());
        }
        OpKind::TomlRemove { toml_path } => {
            remove_toml_path(&mut doc, toml_path);
        }
        OpKind::TomlTransform { rule_id, args } => match rule_id.as_str() {
            "ensure_workspace_resolver_v2" => {
                doc["workspace"]["resolver"] = value("2");
            }
            "set_package_rust_version" => {
                let rust_version = args
                    .as_ref()
                    .and_then(|v| v.get("rust_version"))
                    .and_then(|v| v.as_str())
                    .context("missing rust_version param")?;
                doc["package"]["rust-version"] = value(rust_version);
            }
            "set_package_edition" => {
                let edition = args
                    .as_ref()
                    .and_then(|v| v.get("edition"))
                    .and_then(|v| v.as_str())
                    .context("missing edition param")?;
                doc["package"]["edition"] = value(edition);
            }
            "ensure_path_dep_has_version" => {
                let args = args.as_ref().context("missing args")?;
                let toml_path = args
                    .get("toml_path")
                    .and_then(|v| v.as_array())
                    .context("missing toml_path")?;
                let toml_path: Vec<String> = toml_path
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                let dep_path = args
                    .get("dep_path")
                    .and_then(|v| v.as_str())
                    .context("missing dep_path")?;
                let version = args
                    .get("version")
                    .and_then(|v| v.as_str())
                    .context("missing version param")?;

                let dep_item = get_dep_item_mut(&mut doc, &toml_path)
                    .context("dependency not found at toml_path")?;

                if let Some(inline) = dep_item.as_inline_table_mut() {
                    let current_path = inline.get("path").and_then(|v| v.as_str());
                    if current_path != Some(dep_path) {
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
                    if current_path != Some(dep_path) {
                        return Ok(doc.to_string());
                    }
                    if tbl
                        .get("version")
                        .and_then(|i| i.as_value())
                        .and_then(|v| v.as_str())
                        .is_none()
                    {
                        tbl["version"] = value(version);
                    }
                }
            }
            "use_workspace_dependency" => {
                let args = args.as_ref().context("missing args")?;
                let toml_path = args
                    .get("toml_path")
                    .and_then(|v| v.as_array())
                    .context("missing toml_path")?;
                let toml_path: Vec<String> = toml_path
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();

                let preserved = args.get("preserved");
                let mut inline = InlineTable::new();
                inline.insert("workspace", bool_value(true));
                if let Some(p) = preserved {
                    if let Some(pkg) = p.get("package").and_then(|v| v.as_str()) {
                        inline.insert("package", str_value(pkg));
                    }
                    if let Some(opt) = p.get("optional").and_then(|v| v.as_bool()) {
                        inline.insert("optional", bool_value(opt));
                    }
                    if let Some(df) = p.get("default_features").and_then(|v| v.as_bool()) {
                        inline.insert("default-features", bool_value(df));
                    }
                    if let Some(features) = p.get("features").and_then(|v| v.as_array()) {
                        let mut arr = toml_edit::Array::new();
                        for f in features {
                            if let Some(s) = f.as_str() {
                                arr.push(s);
                            }
                        }
                        inline.insert("features", value(arr).as_value().unwrap().clone());
                    }
                }

                let dep_item = get_dep_item_mut(&mut doc, &toml_path)
                    .context("dependency not found at toml_path")?;
                *dep_item = value(inline);
            }
            _ => {
                // Unknown transform rule; no-op.
            }
        },
    }

    Ok(doc.to_string())
}

/// Execute a plan against pre-loaded file contents (no filesystem access).
///
/// Accepts a `BTreeMap<path, content>` of already-read files and applies each
/// operation in the plan, returning the modified contents. This lets callers
/// read files through a `RepoView` and pass them in without giving the edit
/// engine direct filesystem access.
///
/// The returned map contains only files that were actually changed.
pub fn execute_plan_from_contents(
    before: &BTreeMap<Utf8PathBuf, String>,
    plan: &BuildfixPlan,
    opts: &ApplyOptions,
) -> anyhow::Result<BTreeMap<Utf8PathBuf, String>> {
    let mut current = before.clone();

    for op in &plan.ops {
        let resolved = resolve_op(op, opts);
        if !resolved.allowed {
            continue;
        }

        let file = Utf8PathBuf::from(&op.target.path);
        let old = current.get(&file).cloned().unwrap_or_default();
        let new = apply_op_to_content(&old, &resolved.kind)
            .with_context(|| format!("apply op {} to {}", op.id, op.target.path))?;
        current.insert(file, new);
    }

    // Return only changed files.
    let mut changed = BTreeMap::new();
    for (path, new_content) in &current {
        let old_content = before.get(path).map(|s| s.as_str()).unwrap_or("");
        if new_content != old_content {
            changed.insert(path.clone(), new_content.clone());
        }
    }

    Ok(changed)
}

fn set_toml_path(doc: &mut DocumentMut, toml_path: &[String], value: serde_json::Value) {
    if toml_path.is_empty() {
        return;
    }
    let mut current = doc.as_table_mut();
    for key in &toml_path[..toml_path.len() - 1] {
        current = current
            .entry(key)
            .or_insert(toml_edit::table())
            .as_table_mut()
            .unwrap();
    }
    let last = toml_path.last().unwrap();
    current[last] = Item::Value(json_value_to_toml(value));
}

fn remove_toml_path(doc: &mut DocumentMut, toml_path: &[String]) {
    if toml_path.is_empty() {
        return;
    }
    let mut current = doc.as_table_mut();
    for key in &toml_path[..toml_path.len() - 1] {
        let Some(tbl) = current.get_mut(key).and_then(|i| i.as_table_mut()) else {
            return;
        };
        current = tbl;
    }
    let last = toml_path.last().unwrap();
    current.remove(last);
}

fn json_value_to_toml(json: serde_json::Value) -> toml_edit::Value {
    match json {
        serde_json::Value::String(s) => str_value(&s),
        serde_json::Value::Bool(b) => bool_value(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                value(i).as_value().unwrap().clone()
            } else if let Some(f) = n.as_f64() {
                value(f).as_value().unwrap().clone()
            } else {
                value(n.to_string()).as_value().unwrap().clone()
            }
        }
        serde_json::Value::Array(arr) => {
            let mut out = toml_edit::Array::new();
            for v in arr {
                match v {
                    serde_json::Value::String(s) => out.push(s.as_str()),
                    serde_json::Value::Bool(b) => out.push(b),
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            out.push(i);
                        } else if let Some(f) = n.as_f64() {
                            out.push(f);
                        }
                    }
                    _ => {}
                }
            }
            value(out).as_value().unwrap().clone()
        }
        _ => value("").as_value().unwrap().clone(),
    }
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

/// Checks if an apply result indicates a policy block.
pub fn check_policy_block(apply: &BuildfixApply, was_dry_run: bool) -> Option<PolicyBlockError> {
    if was_dry_run {
        return None;
    }

    if !apply.preconditions.verified {
        return Some(PolicyBlockError::PreconditionMismatch {
            message: "precondition mismatch".to_string(),
        });
    }

    let blocked: Vec<&ApplyResult> = apply
        .results
        .iter()
        .filter(|r| r.status == ApplyStatus::Blocked)
        .collect();

    if !blocked.is_empty() {
        let reasons: Vec<String> = blocked
            .iter()
            .filter_map(|r| r.blocked_reason.clone())
            .collect();

        if reasons.iter().any(|r| r.contains("safety")) {
            return Some(PolicyBlockError::SafetyGateDenial {
                message: format!("{} op(s) blocked by safety gate", blocked.len()),
            });
        }

        return Some(PolicyBlockError::PolicyDenial {
            message: format!("{} op(s) blocked by policy", blocked.len()),
        });
    }

    if apply.summary.failed > 0 {
        return Some(PolicyBlockError::PreconditionMismatch {
            message: format!("{} op(s) failed", apply.summary.failed),
        });
    }

    None
}
