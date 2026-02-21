//! Artifact serialization and persistence for buildfix outputs.

use anyhow::Context;
use buildfix_render::{render_apply_md, render_comment_md, render_plan_md};
use buildfix_types::apply::BuildfixApply;
use buildfix_types::plan::BuildfixPlan;
use buildfix_types::report::BuildfixReport;
use buildfix_types::schema::BUILDFIX_REPORT_V1;
use buildfix_types::wire::{PlanV1, ReportV1};
use camino::Utf8Path;
use std::collections::BTreeMap;
use std::fs;

/// Filesystem-facing abstraction for artifact emission.
pub trait ArtifactWriter {
    fn write_file(&self, path: &Utf8Path, contents: &[u8]) -> anyhow::Result<()>;
    fn create_dir_all(&self, path: &Utf8Path) -> anyhow::Result<()>;
}

/// Standard filesystem implementation.
#[derive(Debug, Default, Clone)]
pub struct FsArtifactWriter;

impl ArtifactWriter for FsArtifactWriter {
    fn write_file(&self, path: &Utf8Path, contents: &[u8]) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create parent dir for {}", path))?;
        }
        fs::write(path, contents).with_context(|| format!("write {}", path))
    }

    fn create_dir_all(&self, path: &Utf8Path) -> anyhow::Result<()> {
        fs::create_dir_all(path).with_context(|| format!("create dir {}", path))
    }
}

fn render_plan_report(
    plan: &BuildfixPlan,
    report: &BuildfixReport,
    patch: &str,
    out_dir: &Utf8Path,
) -> anyhow::Result<BTreeMap<String, Vec<u8>>> {
    let plan_wire = PlanV1::try_from(plan).context("convert plan to wire")?;
    let plan_json = serde_json::to_string_pretty(&plan_wire).context("serialize plan")?;

    let report_wire = ReportV1::from(report);
    let report_json = serde_json::to_string_pretty(&report_wire).context("serialize report")?;

    let mut extra_report = report.clone();
    extra_report.schema = BUILDFIX_REPORT_V1.to_string();
    let extras_wire = ReportV1::from(&extra_report);
    let extras_json =
        serde_json::to_string_pretty(&extras_wire).context("serialize extras report")?;

    let mut files = BTreeMap::new();
    files.insert(
        out_dir.join("plan.json").to_string(),
        plan_json.into_bytes(),
    );
    files.insert(
        out_dir.join("plan.md").to_string(),
        render_plan_md(plan).into_bytes(),
    );
    files.insert(
        out_dir.join("comment.md").to_string(),
        render_comment_md(plan).into_bytes(),
    );
    files.insert(
        out_dir.join("patch.diff").to_string(),
        patch.as_bytes().to_vec(),
    );
    files.insert(
        out_dir.join("report.json").to_string(),
        report_json.into_bytes(),
    );
    files.insert(
        out_dir
            .join("extras")
            .join("buildfix.report.v1.json")
            .to_string(),
        extras_json.into_bytes(),
    );
    Ok(files)
}

fn render_apply_report(
    apply: &BuildfixApply,
    report: &BuildfixReport,
    patch: &str,
    out_dir: &Utf8Path,
) -> anyhow::Result<BTreeMap<String, Vec<u8>>> {
    let apply_wire =
        buildfix_types::wire::ApplyV1::try_from(apply).context("convert apply to wire")?;
    let apply_json = serde_json::to_string_pretty(&apply_wire).context("serialize apply")?;

    let report_wire = ReportV1::from(report);
    let report_json = serde_json::to_string_pretty(&report_wire).context("serialize report")?;

    let mut extra_report = report.clone();
    extra_report.schema = BUILDFIX_REPORT_V1.to_string();
    let extras_wire = ReportV1::from(&extra_report);
    let extras_json =
        serde_json::to_string_pretty(&extras_wire).context("serialize extras report")?;

    let mut files = BTreeMap::new();
    files.insert(
        out_dir.join("apply.json").to_string(),
        apply_json.into_bytes(),
    );
    files.insert(
        out_dir.join("apply.md").to_string(),
        render_apply_md(apply).into_bytes(),
    );
    files.insert(
        out_dir.join("patch.diff").to_string(),
        patch.as_bytes().to_vec(),
    );
    files.insert(
        out_dir.join("report.json").to_string(),
        report_json.into_bytes(),
    );
    files.insert(
        out_dir
            .join("extras")
            .join("buildfix.report.v1.json")
            .to_string(),
        extras_json.into_bytes(),
    );
    Ok(files)
}

fn write_files<W: ArtifactWriter>(
    files: BTreeMap<String, Vec<u8>>,
    writer: &W,
) -> anyhow::Result<()> {
    for (path, contents) in files {
        writer.write_file(Utf8Path::new(&path), &contents)?;
    }
    Ok(())
}

/// Emit all plan artifacts (plan.json, plan.md, comment.md, patch, report, extras).
pub fn write_plan_artifacts<W: ArtifactWriter>(
    plan: &BuildfixPlan,
    report: &BuildfixReport,
    patch: &str,
    out_dir: &Utf8Path,
    writer: &W,
) -> anyhow::Result<()> {
    writer.create_dir_all(out_dir)?;
    writer.create_dir_all(&out_dir.join("extras"))?;
    let files = render_plan_report(plan, report, patch, out_dir)?;
    write_files(files, writer)
}

/// Emit all apply artifacts (apply.json, apply.md, patch, report, extras).
pub fn write_apply_artifacts<W: ArtifactWriter>(
    apply: &BuildfixApply,
    report: &BuildfixReport,
    patch: &str,
    out_dir: &Utf8Path,
    writer: &W,
) -> anyhow::Result<()> {
    writer.create_dir_all(out_dir)?;
    writer.create_dir_all(&out_dir.join("extras"))?;
    let files = render_apply_report(apply, report, patch, out_dir)?;
    write_files(files, writer)
}
