//! Integration tests for buildfix-core crate.
//!
//! These tests verify the public API surface, settings configuration,
//! port/adapter patterns, and pipeline orchestration behavior.

use buildfix_core::RepoView;
use buildfix_core::ports::{GitPort, ReceiptSource, WritePort};
use buildfix_core::settings::{ApplySettings, PlanSettings, RunMode};
use buildfix_receipts::{LoadedReceipt, ReceiptLoadError};
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{PlanOp, PlanPolicy, Rationale, RepoInfo};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, RunInfo, ToolInfo, Verdict};
use buildfix_types::wire::PlanV1;
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::HashMap;
use std::sync::Mutex;
use tempfile::TempDir;

// =============================================================================
// Settings Tests
// =============================================================================

mod settings_tests {
    use super::*;

    #[test]
    fn plan_settings_default_has_expected_values() {
        let settings = PlanSettings::default();

        assert_eq!(settings.repo_root.as_str(), ".");
        assert_eq!(settings.artifacts_dir.as_str(), "artifacts");
        assert_eq!(settings.out_dir.as_str(), "artifacts/buildfix");
        assert!(settings.allow.is_empty());
        assert!(settings.deny.is_empty());
        assert!(!settings.allow_guarded);
        assert!(!settings.allow_unsafe);
        assert!(!settings.allow_dirty);
        assert!(settings.max_ops.is_none());
        assert!(settings.max_files.is_none());
        assert!(settings.max_patch_bytes.is_none());
        assert!(settings.params.is_empty());
        assert!(settings.require_clean_hashes);
        assert!(!settings.git_head_precondition);
        assert_eq!(settings.backup_suffix, ".buildfix.bak");
        assert_eq!(settings.mode, RunMode::Standalone);
    }

    #[test]
    fn apply_settings_default_has_expected_values() {
        let settings = ApplySettings::default();

        assert_eq!(settings.repo_root.as_str(), ".");
        assert_eq!(settings.out_dir.as_str(), "artifacts/buildfix");
        assert!(settings.dry_run);
        assert!(!settings.allow_guarded);
        assert!(!settings.allow_unsafe);
        assert!(!settings.allow_dirty);
        assert!(settings.params.is_empty());
        assert!(!settings.auto_commit);
        assert!(settings.commit_message.is_none());
        assert!(settings.backup_enabled);
        assert_eq!(settings.backup_suffix, ".buildfix.bak");
        assert_eq!(settings.mode, RunMode::Standalone);
    }

    #[test]
    fn plan_settings_can_be_customized() {
        let mut params = HashMap::new();
        params.insert("version".to_string(), "1.0.0".to_string());

        let settings = PlanSettings {
            repo_root: Utf8PathBuf::from("/custom/root"),
            artifacts_dir: Utf8PathBuf::from("/custom/artifacts"),
            out_dir: Utf8PathBuf::from("/custom/out"),
            allow: vec!["fix1".to_string(), "fix2".to_string()],
            deny: vec!["fix3".to_string()],
            allow_guarded: true,
            allow_unsafe: true,
            allow_dirty: true,
            max_ops: Some(100),
            max_files: Some(10),
            max_patch_bytes: Some(1024),
            params,
            require_clean_hashes: false,
            git_head_precondition: true,
            backup_suffix: ".bak".to_string(),
            mode: RunMode::Cockpit,
        };

        assert_eq!(settings.repo_root.as_str(), "/custom/root");
        assert_eq!(settings.artifacts_dir.as_str(), "/custom/artifacts");
        assert_eq!(settings.out_dir.as_str(), "/custom/out");
        assert_eq!(settings.allow.len(), 2);
        assert_eq!(settings.deny.len(), 1);
        assert!(settings.allow_guarded);
        assert!(settings.allow_unsafe);
        assert!(settings.allow_dirty);
        assert_eq!(settings.max_ops, Some(100));
        assert_eq!(settings.max_files, Some(10));
        assert_eq!(settings.max_patch_bytes, Some(1024));
        assert_eq!(settings.params.len(), 1);
        assert!(!settings.require_clean_hashes);
        assert!(settings.git_head_precondition);
        assert_eq!(settings.backup_suffix, ".bak");
        assert_eq!(settings.mode, RunMode::Cockpit);
    }

    #[test]
    fn apply_settings_can_be_customized() {
        let mut params = HashMap::new();
        params.insert("key".to_string(), "value".to_string());

        let settings = ApplySettings {
            repo_root: Utf8PathBuf::from("/repo"),
            out_dir: Utf8PathBuf::from("/out"),
            dry_run: false,
            allow_guarded: true,
            allow_unsafe: true,
            allow_dirty: true,
            params,
            auto_commit: true,
            commit_message: Some("custom message".to_string()),
            backup_enabled: false,
            backup_suffix: ".backup".to_string(),
            mode: RunMode::Cockpit,
        };

        assert_eq!(settings.repo_root.as_str(), "/repo");
        assert_eq!(settings.out_dir.as_str(), "/out");
        assert!(!settings.dry_run);
        assert!(settings.allow_guarded);
        assert!(settings.allow_unsafe);
        assert!(settings.allow_dirty);
        assert_eq!(settings.params.len(), 1);
        assert!(settings.auto_commit);
        assert_eq!(settings.commit_message.as_deref(), Some("custom message"));
        assert!(!settings.backup_enabled);
        assert_eq!(settings.backup_suffix, ".backup");
        assert_eq!(settings.mode, RunMode::Cockpit);
    }

    #[test]
    fn run_mode_default_is_standalone() {
        let mode = RunMode::default();
        assert_eq!(mode, RunMode::Standalone);
    }

    #[test]
    fn run_mode_equality() {
        assert_eq!(RunMode::Standalone, RunMode::Standalone);
        assert_eq!(RunMode::Cockpit, RunMode::Cockpit);
        assert_ne!(RunMode::Standalone, RunMode::Cockpit);
    }

    #[test]
    fn run_mode_debug_impl() {
        let standalone = RunMode::Standalone;
        let cockpit = RunMode::Cockpit;

        assert!(format!("{:?}", standalone).contains("Standalone"));
        assert!(format!("{:?}", cockpit).contains("Cockpit"));
    }
}

// =============================================================================
// Port Trait Tests (Mock Implementations)
// =============================================================================

mod port_tests {
    use super::*;

    /// Mock GitPort for testing
    struct MockGitPort {
        head_sha: Option<String>,
        is_dirty: Option<bool>,
        commit_result: Option<String>,
        commit_calls: Mutex<usize>,
    }

    impl MockGitPort {
        fn new(head: Option<String>, dirty: Option<bool>) -> Self {
            Self {
                head_sha: head,
                is_dirty: dirty,
                commit_result: None,
                commit_calls: Mutex::new(0),
            }
        }

        fn with_commit_result(mut self, result: Option<String>) -> Self {
            self.commit_result = result;
            self
        }

        fn commit_call_count(&self) -> usize {
            *self.commit_calls.lock().unwrap()
        }
    }

    impl GitPort for MockGitPort {
        fn head_sha(&self, _repo_root: &Utf8Path) -> anyhow::Result<Option<String>> {
            Ok(self.head_sha.clone())
        }

        fn is_dirty(&self, _repo_root: &Utf8Path) -> anyhow::Result<Option<bool>> {
            Ok(self.is_dirty)
        }

        fn commit_all(
            &self,
            _repo_root: &Utf8Path,
            _message: &str,
        ) -> anyhow::Result<Option<String>> {
            *self.commit_calls.lock().unwrap() += 1;
            Ok(self.commit_result.clone())
        }
    }

    /// Mock ReceiptSource for testing
    struct MockReceiptSource {
        receipts: Vec<LoadedReceipt>,
        should_fail: bool,
    }

    impl MockReceiptSource {
        fn new(receipts: Vec<LoadedReceipt>) -> Self {
            Self {
                receipts,
                should_fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                receipts: vec![],
                should_fail: true,
            }
        }
    }

    impl ReceiptSource for MockReceiptSource {
        fn load_receipts(&self) -> anyhow::Result<Vec<LoadedReceipt>> {
            if self.should_fail {
                anyhow::bail!("mock receipt load failure")
            }
            Ok(self.receipts.clone())
        }
    }

    /// Mock WritePort for testing
    #[derive(Default)]
    struct MockWritePort {
        files: Mutex<HashMap<String, Vec<u8>>>,
        dirs: Mutex<Vec<String>>,
    }

    impl MockWritePort {
        fn new() -> Self {
            Self::default()
        }

        fn get_file(&self, path: &str) -> Option<Vec<u8>> {
            self.files.lock().unwrap().get(path).cloned()
        }

        fn file_exists(&self, path: &str) -> bool {
            self.files.lock().unwrap().contains_key(path)
        }

        fn dir_count(&self) -> usize {
            self.dirs.lock().unwrap().len()
        }
    }

    impl WritePort for MockWritePort {
        fn write_file(&self, path: &Utf8Path, contents: &[u8]) -> anyhow::Result<()> {
            let key = path.as_str().replace('\\', "/");
            self.files.lock().unwrap().insert(key, contents.to_vec());
            Ok(())
        }

        fn create_dir_all(&self, path: &Utf8Path) -> anyhow::Result<()> {
            let key = path.as_str().replace('\\', "/");
            self.dirs.lock().unwrap().push(key);
            Ok(())
        }
    }

    #[test]
    fn mock_git_port_returns_configured_head_sha() {
        let git = MockGitPort::new(Some("abc123".to_string()), None);

        let result = git.head_sha(Utf8Path::new(".")).unwrap();
        assert_eq!(result, Some("abc123".to_string()));
    }

    #[test]
    fn mock_git_port_returns_none_when_no_head() {
        let git = MockGitPort::new(None, None);

        let result = git.head_sha(Utf8Path::new(".")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn mock_git_port_returns_configured_dirty_status() {
        let git_clean = MockGitPort::new(None, Some(false));
        let git_dirty = MockGitPort::new(None, Some(true));
        let git_unknown = MockGitPort::new(None, None);

        assert_eq!(git_clean.is_dirty(Utf8Path::new(".")).unwrap(), Some(false));
        assert_eq!(git_dirty.is_dirty(Utf8Path::new(".")).unwrap(), Some(true));
        assert_eq!(git_unknown.is_dirty(Utf8Path::new(".")).unwrap(), None);
    }

    #[test]
    fn mock_git_port_commit_all_tracks_calls() {
        let git =
            MockGitPort::new(None, Some(false)).with_commit_result(Some("commit123".to_string()));

        let result = git.commit_all(Utf8Path::new("."), "test message").unwrap();
        assert_eq!(result, Some("commit123".to_string()));
        assert_eq!(git.commit_call_count(), 1);

        git.commit_all(Utf8Path::new("."), "another message")
            .unwrap();
        assert_eq!(git.commit_call_count(), 2);
    }

    #[test]
    fn mock_receipt_source_returns_configured_receipts() {
        let receipts = vec![create_stub_receipt("artifacts/test/report.json")];
        let source = MockReceiptSource::new(receipts);

        let loaded = source.load_receipts().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].path.as_str(), "artifacts/test/report.json");
    }

    #[test]
    fn mock_receipt_source_can_fail() {
        let source = MockReceiptSource::failing();

        let result = source.load_receipts();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("mock receipt load failure")
        );
    }

    #[test]
    fn mock_write_port_stores_files() {
        let writer = MockWritePort::new();

        writer
            .write_file(Utf8Path::new("out/test.json"), b"content")
            .unwrap();

        assert!(writer.file_exists("out/test.json"));
        assert_eq!(writer.get_file("out/test.json"), Some(b"content".to_vec()));
    }

    #[test]
    fn mock_write_port_tracks_directories() {
        let writer = MockWritePort::new();

        writer.create_dir_all(Utf8Path::new("out/subdir")).unwrap();
        writer.create_dir_all(Utf8Path::new("out/another")).unwrap();

        assert_eq!(writer.dir_count(), 2);
    }

    #[test]
    fn mock_write_port_overwrites_files() {
        let writer = MockWritePort::new();

        writer
            .write_file(Utf8Path::new("test.txt"), b"first")
            .unwrap();
        writer
            .write_file(Utf8Path::new("test.txt"), b"second")
            .unwrap();

        assert_eq!(writer.get_file("test.txt"), Some(b"second".to_vec()));
    }

    // Test that trait objects work correctly (dynamic dispatch)
    #[test]
    fn git_port_trait_object_works() {
        let git: Box<dyn GitPort> = Box::new(MockGitPort::new(Some("sha".to_string()), Some(true)));

        let head = git.head_sha(Utf8Path::new(".")).unwrap();
        let dirty = git.is_dirty(Utf8Path::new(".")).unwrap();

        assert_eq!(head, Some("sha".to_string()));
        assert_eq!(dirty, Some(true));
    }

    #[test]
    fn receipt_source_trait_object_works() {
        let source: Box<dyn ReceiptSource> = Box::new(MockReceiptSource::new(vec![
            create_stub_receipt("a.json"),
            create_stub_receipt("b.json"),
        ]));

        let receipts = source.load_receipts().unwrap();
        assert_eq!(receipts.len(), 2);
    }

    #[test]
    fn write_port_trait_object_works() {
        let writer: Box<dyn WritePort> = Box::new(MockWritePort::new());

        writer
            .write_file(Utf8Path::new("test.txt"), b"data")
            .unwrap();
        writer.create_dir_all(Utf8Path::new("dir")).unwrap();
    }
}

// =============================================================================
// Re-exports Tests
// =============================================================================

mod reexport_tests {
    use super::*;

    #[test]
    fn repo_view_trait_is_reexported() {
        // Verify that RepoView is accessible from buildfix_core
        fn _assert_repo_view_trait_available<T: RepoView>() {}
    }

    #[test]
    fn builtin_fixer_metas_is_reexported() {
        // Verify the function is accessible
        let _metas = buildfix_core::builtin_fixer_metas();
    }

    #[test]
    fn loaded_receipt_is_reexported() {
        // Verify LoadedReceipt is accessible
        let _receipt: Option<buildfix_core::LoadedReceipt> = None;
    }

    #[test]
    fn receipt_envelope_is_reexported() {
        // Verify ReceiptEnvelope is accessible
        let _envelope: Option<buildfix_core::ReceiptEnvelope> = None;
    }

    #[test]
    fn receipt_load_error_is_reexported() {
        // Verify ReceiptLoadError is accessible
        let _error: Option<buildfix_core::ReceiptLoadError> = None;
    }
}

// =============================================================================
// Pipeline Integration Tests
// =============================================================================

mod pipeline_tests {
    use super::*;
    use buildfix_core::adapters::InMemoryReceiptSource;
    use buildfix_core::pipeline::{ToolError, run_apply, run_plan};

    /// Stub GitPort for pipeline tests
    struct StubGitPort {
        head: Option<String>,
        dirty: Option<bool>,
    }

    impl Default for StubGitPort {
        fn default() -> Self {
            Self {
                head: None,
                dirty: Some(false),
            }
        }
    }

    impl GitPort for StubGitPort {
        fn head_sha(&self, _repo_root: &Utf8Path) -> anyhow::Result<Option<String>> {
            Ok(self.head.clone())
        }

        fn is_dirty(&self, _repo_root: &Utf8Path) -> anyhow::Result<Option<bool>> {
            Ok(self.dirty)
        }
    }

    /// Stub WritePort for pipeline tests
    #[derive(Default)]
    struct MemWritePort {
        files: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl WritePort for MemWritePort {
        fn write_file(&self, path: &Utf8Path, contents: &[u8]) -> anyhow::Result<()> {
            let key = path.as_str().replace('\\', "/");
            self.files.lock().unwrap().insert(key, contents.to_vec());
            Ok(())
        }

        fn create_dir_all(&self, _path: &Utf8Path) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn tool_info() -> ToolInfo {
        ToolInfo {
            name: "buildfix".into(),
            version: Some("0.0.0-test".into()),
            repo: None,
            commit: None,
        }
    }

    fn create_temp_repo(manifest: &str) -> (TempDir, Utf8PathBuf) {
        let temp = TempDir::new().unwrap();
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();
        std::fs::write(root.join("Cargo.toml"), manifest).unwrap();
        (temp, root)
    }

    fn resolver_receipt() -> LoadedReceipt {
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: ToolInfo {
                name: "builddiag".to_string(),
                version: Some("1.0.0".to_string()),
                repo: None,
                commit: None,
            },
            run: RunInfo::default(),
            verdict: Verdict::default(),
            findings: vec![Finding {
                severity: Default::default(),
                check_id: Some("workspace.resolver_v2".to_string()),
                code: Some("RESOLVER".to_string()),
                message: None,
                location: Some(Location {
                    path: Utf8PathBuf::from("Cargo.toml"),
                    line: Some(1),
                    column: None,
                }),
                fingerprint: None,
                data: None,
                ..Default::default()
            }],
            capabilities: None,
            data: None,
        };

        LoadedReceipt {
            path: Utf8PathBuf::from("artifacts/builddiag/report.json"),
            sensor_id: "builddiag".to_string(),
            receipt: Ok(receipt),
        }
    }

    fn build_plan_settings(root: &Utf8Path) -> PlanSettings {
        PlanSettings {
            repo_root: root.to_path_buf(),
            artifacts_dir: root.join("artifacts"),
            out_dir: root.join("artifacts/buildfix"),
            allow: Vec::new(),
            deny: Vec::new(),
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: None,
            max_files: None,
            max_patch_bytes: None,
            params: HashMap::new(),
            require_clean_hashes: true,
            git_head_precondition: false,
            backup_suffix: ".buildfix.bak".to_string(),
            mode: RunMode::Standalone,
        }
    }

    fn make_apply_settings(root: &Utf8Path, out_dir: &Utf8Path) -> ApplySettings {
        ApplySettings {
            repo_root: root.to_path_buf(),
            out_dir: out_dir.to_path_buf(),
            dry_run: true,
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            params: HashMap::new(),
            auto_commit: false,
            commit_message: None,
            backup_enabled: false,
            backup_suffix: ".buildfix.bak".to_string(),
            mode: RunMode::Standalone,
        }
    }

    fn make_plan(ops: Vec<PlanOp>) -> BuildfixPlan {
        let mut plan = BuildfixPlan::new(
            tool_info(),
            RepoInfo {
                root: ".".into(),
                head_sha: None,
                dirty: None,
            },
            PlanPolicy::default(),
        );
        plan.summary.ops_total = ops.len() as u64;
        plan.ops = ops;
        plan
    }

    fn make_op(safety: SafetyClass, blocked: bool) -> PlanOp {
        PlanOp {
            id: "test-op".into(),
            safety,
            blocked,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: "Cargo.toml".into(),
            },
            kind: OpKind::TomlSet {
                toml_path: vec!["workspace".into(), "resolver".into()],
                value: serde_json::json!("2"),
            },
            rationale: Rationale {
                fix_key: "test".into(),
                description: None,
                findings: vec![],
            },
            params_required: vec![],
            preview: None,
        }
    }

    use buildfix_types::plan::BuildfixPlan;

    #[test]
    fn plan_outcome_contains_expected_fields() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let receipts = InMemoryReceiptSource::new(vec![resolver_receipt()]);
        let settings = build_plan_settings(&root);
        let git = StubGitPort::default();

        let outcome = run_plan(&settings, &receipts, &git, tool_info()).unwrap();

        // Plan should have ops
        assert!(!outcome.plan.ops.is_empty());

        // Report should be generated
        assert_eq!(outcome.report.tool.name, "buildfix");

        // Patch should be non-empty (dry-run preview)
        assert!(!outcome.patch.is_empty());

        // No policy block for safe ops
        assert!(!outcome.policy_block);
    }

    #[test]
    fn plan_outcome_git_info_populated() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let receipts = InMemoryReceiptSource::new(vec![resolver_receipt()]);
        let settings = build_plan_settings(&root);
        let git = StubGitPort {
            head: Some("deadbeef".to_string()),
            dirty: Some(false),
        };

        let outcome = run_plan(&settings, &receipts, &git, tool_info()).unwrap();

        assert_eq!(outcome.plan.repo.head_sha.as_deref(), Some("deadbeef"));
        assert_eq!(outcome.plan.repo.dirty, Some(false));
    }

    #[test]
    fn plan_outcome_preconditions_attached() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let receipts = InMemoryReceiptSource::new(vec![resolver_receipt()]);
        let settings = build_plan_settings(&root);
        let git = StubGitPort::default();

        let outcome = run_plan(&settings, &receipts, &git, tool_info()).unwrap();

        // Should have file preconditions
        assert!(!outcome.plan.preconditions.files.is_empty());

        // Verify SHA256 is populated
        let pre = &outcome.plan.preconditions.files[0];
        assert_eq!(pre.path, "Cargo.toml");
        assert!(!pre.sha256.is_empty());
    }

    #[test]
    fn plan_outcome_respects_max_ops_cap() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let receipts = InMemoryReceiptSource::new(vec![resolver_receipt()]);

        let mut settings = build_plan_settings(&root);
        settings.max_ops = Some(0); // Block all ops

        let git = StubGitPort::default();
        let outcome = run_plan(&settings, &receipts, &git, tool_info()).unwrap();

        // All ops should be blocked
        assert!(outcome.plan.ops.iter().all(|o| o.blocked));
        assert!(outcome.policy_block);
    }

    #[test]
    fn tool_error_policy_block_display() {
        let err = ToolError::PolicyBlock;
        assert_eq!(err.to_string(), "policy block");
    }

    #[test]
    fn tool_error_internal_display() {
        let err = ToolError::Internal(anyhow::anyhow!("something went wrong"));
        let msg = err.to_string();
        assert!(msg.contains("something went wrong"));
    }

    #[test]
    fn write_plan_artifacts_creates_expected_files() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let receipts = InMemoryReceiptSource::new(vec![resolver_receipt()]);
        let settings = build_plan_settings(&root);
        let git = StubGitPort::default();

        let outcome = run_plan(&settings, &receipts, &git, tool_info()).unwrap();

        let writer = MemWritePort::default();
        let out_dir = Utf8PathBuf::from("out");
        buildfix_core::pipeline::write_plan_artifacts(&outcome, &out_dir, &writer).unwrap();

        let files = writer.files.lock().unwrap();
        assert!(files.contains_key("out/plan.json"));
        assert!(files.contains_key("out/plan.md"));
        assert!(files.contains_key("out/comment.md"));
        assert!(files.contains_key("out/patch.diff"));
        assert!(files.contains_key("out/report.json"));
    }

    #[test]
    fn apply_outcome_with_dry_run() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let out_dir = root.join("artifacts").join("buildfix");
        std::fs::create_dir_all(&out_dir).unwrap();

        // Create a plan file
        let plan = make_plan(vec![make_op(SafetyClass::Safe, false)]);
        let plan_wire = PlanV1::try_from(&plan).unwrap();
        let plan_json = serde_json::to_string_pretty(&plan_wire).unwrap();
        std::fs::write(out_dir.join("plan.json"), plan_json).unwrap();

        let settings = make_apply_settings(&root, &out_dir);
        let git = StubGitPort::default();

        let outcome = run_apply(&settings, &git, tool_info()).unwrap();

        // Dry-run should skip the op
        assert_eq!(outcome.apply.summary.applied, 0);
        assert!(!outcome.patch.is_empty());
    }

    #[test]
    fn apply_blocks_on_dirty_tree() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let out_dir = root.join("artifacts").join("buildfix");
        std::fs::create_dir_all(&out_dir).unwrap();

        let plan = make_plan(vec![make_op(SafetyClass::Safe, false)]);
        let plan_wire = PlanV1::try_from(&plan).unwrap();
        let plan_json = serde_json::to_string_pretty(&plan_wire).unwrap();
        std::fs::write(out_dir.join("plan.json"), plan_json).unwrap();

        let mut settings = make_apply_settings(&root, &out_dir);
        settings.dry_run = false; // Real apply

        let git = StubGitPort {
            head: Some("abc".to_string()),
            dirty: Some(true), // Dirty tree
        };

        let outcome = run_apply(&settings, &git, tool_info()).unwrap();

        assert!(outcome.policy_block);
        assert!(outcome.patch.is_empty());
    }

    #[test]
    fn apply_allows_dirty_tree_with_flag() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let out_dir = root.join("artifacts").join("buildfix");
        std::fs::create_dir_all(&out_dir).unwrap();

        let plan = make_plan(vec![make_op(SafetyClass::Safe, false)]);
        let plan_wire = PlanV1::try_from(&plan).unwrap();
        let plan_json = serde_json::to_string_pretty(&plan_wire).unwrap();
        std::fs::write(out_dir.join("plan.json"), plan_json).unwrap();

        let mut settings = make_apply_settings(&root, &out_dir);
        settings.dry_run = false;
        settings.allow_dirty = true;

        let git = StubGitPort {
            head: Some("abc".to_string()),
            dirty: Some(true),
        };

        let outcome = run_apply(&settings, &git, tool_info()).unwrap();

        assert!(!outcome.policy_block);
    }

    #[test]
    fn apply_fails_without_plan_file() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let out_dir = root.join("artifacts").join("buildfix");
        std::fs::create_dir_all(&out_dir).unwrap();
        // No plan.json created

        let settings = make_apply_settings(&root, &out_dir);
        let git = StubGitPort::default();

        let result = run_apply(&settings, &git, tool_info());
        assert!(result.is_err());

        match result.unwrap_err() {
            ToolError::Internal(e) => {
                assert!(e.to_string().contains("read"));
            }
            ToolError::PolicyBlock => panic!("expected internal error"),
        }
    }

    #[test]
    fn apply_fails_with_invalid_plan_json() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let out_dir = root.join("artifacts").join("buildfix");
        std::fs::create_dir_all(&out_dir).unwrap();
        std::fs::write(out_dir.join("plan.json"), "not valid json").unwrap();

        let settings = make_apply_settings(&root, &out_dir);
        let git = StubGitPort::default();

        let result = run_apply(&settings, &git, tool_info());
        assert!(result.is_err());

        match result.unwrap_err() {
            ToolError::Internal(e) => {
                assert!(e.to_string().contains("parse"));
            }
            ToolError::PolicyBlock => panic!("expected internal error"),
        }
    }

    #[test]
    fn write_apply_artifacts_creates_expected_files() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let out_dir = root.join("artifacts").join("buildfix");
        std::fs::create_dir_all(&out_dir).unwrap();

        let plan = make_plan(vec![make_op(SafetyClass::Safe, false)]);
        let plan_wire = PlanV1::try_from(&plan).unwrap();
        let plan_json = serde_json::to_string_pretty(&plan_wire).unwrap();
        std::fs::write(out_dir.join("plan.json"), plan_json).unwrap();

        let settings = make_apply_settings(&root, &out_dir);
        let git = StubGitPort::default();

        let outcome = run_apply(&settings, &git, tool_info()).unwrap();

        let writer = MemWritePort::default();
        let out_dir = Utf8PathBuf::from("out");
        buildfix_core::pipeline::write_apply_artifacts(&outcome, &out_dir, &writer).unwrap();

        let files = writer.files.lock().unwrap();
        assert!(files.contains_key("out/apply.json"));
        assert!(files.contains_key("out/apply.md"));
        assert!(files.contains_key("out/patch.diff"));
        assert!(files.contains_key("out/report.json"));
    }
}

// =============================================================================
// Edge Cases and Error Handling
// =============================================================================

mod edge_case_tests {
    use super::*;

    #[test]
    fn empty_params_map_is_valid() {
        let settings = PlanSettings {
            params: HashMap::new(),
            ..PlanSettings::default()
        };
        assert!(settings.params.is_empty());
    }

    #[test]
    fn params_map_can_contain_multiple_entries() {
        let mut params = HashMap::new();
        params.insert("key1".to_string(), "value1".to_string());
        params.insert("key2".to_string(), "value2".to_string());
        params.insert("key3".to_string(), "value3".to_string());

        let settings = PlanSettings {
            params,
            ..PlanSettings::default()
        };
        assert_eq!(settings.params.len(), 3);
    }

    #[test]
    fn allow_deny_lists_can_overlap() {
        // The behavior when allow and deny overlap is defined by the planner
        let settings = PlanSettings {
            allow: vec!["fix1".to_string(), "fix2".to_string()],
            deny: vec!["fix2".to_string(), "fix3".to_string()],
            ..PlanSettings::default()
        };

        assert!(settings.allow.contains(&"fix2".to_string()));
        assert!(settings.deny.contains(&"fix2".to_string()));
    }

    #[test]
    fn backup_suffix_can_be_customized() {
        let settings = PlanSettings {
            backup_suffix: ".custom_backup".to_string(),
            ..PlanSettings::default()
        };
        assert_eq!(settings.backup_suffix, ".custom_backup");
    }

    #[test]
    fn cockpit_mode_can_be_set() {
        let plan_settings = PlanSettings {
            mode: RunMode::Cockpit,
            ..PlanSettings::default()
        };
        let apply_settings = ApplySettings {
            mode: RunMode::Cockpit,
            ..ApplySettings::default()
        };

        assert_eq!(plan_settings.mode, RunMode::Cockpit);
        assert_eq!(apply_settings.mode, RunMode::Cockpit);
    }

    #[test]
    fn commit_message_can_be_set() {
        let settings = ApplySettings {
            commit_message: Some("chore: auto-fix via buildfix".to_string()),
            ..ApplySettings::default()
        };

        assert_eq!(
            settings.commit_message,
            Some("chore: auto-fix via buildfix".to_string())
        );
    }

    #[test]
    fn max_caps_can_be_set_to_zero() {
        let settings = PlanSettings {
            max_ops: Some(0),
            max_files: Some(0),
            max_patch_bytes: Some(0),
            ..PlanSettings::default()
        };

        assert_eq!(settings.max_ops, Some(0));
        assert_eq!(settings.max_files, Some(0));
        assert_eq!(settings.max_patch_bytes, Some(0));
    }

    #[test]
    fn allow_unsafe_is_independent_of_allow_guarded() {
        let settings = ApplySettings {
            allow_guarded: true,
            allow_unsafe: false,
            ..ApplySettings::default()
        };
        assert!(settings.allow_guarded);
        assert!(!settings.allow_unsafe);

        let settings = ApplySettings {
            allow_guarded: false,
            allow_unsafe: true,
            ..ApplySettings::default()
        };
        assert!(!settings.allow_guarded);
        assert!(settings.allow_unsafe);
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

fn create_stub_receipt(path: &str) -> LoadedReceipt {
    LoadedReceipt {
        path: Utf8PathBuf::from(path),
        sensor_id: "test".to_string(),
        receipt: Err(ReceiptLoadError::Io {
            message: "stub receipt".to_string(),
        }),
    }
}
