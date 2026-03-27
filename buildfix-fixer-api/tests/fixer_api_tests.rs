//! Comprehensive unit tests for buildfix-fixer-api
//!
//! Tests cover:
//! - Fixer trait contract
//! - FixerMeta creation and serialization
//! - PlannerConfig defaults and configuration
//! - PlanContext creation
//! - RepoView trait mock implementation
//! - MatchedFinding fields and construction
//! - ReceiptSet edge cases and additional scenarios
//! - SafetyClass integration

use buildfix_fixer_api::{
    Fixer, FixerMeta, MatchedFinding, PlanContext, PlannerConfig, ReceiptSet, RepoView,
};
use buildfix_receipts::LoadedReceipt;
use buildfix_types::ops::{OpKind, OpPreview, OpTarget, SafetyClass};
use buildfix_types::plan::{FindingRef, PlanOp, Rationale};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, ToolInfo};
use camino::Utf8Path;
use std::collections::HashMap;

// =============================================================================
// Mock Implementations for Testing
// =============================================================================

/// Mock implementation of RepoView for testing
struct MockRepoView {
    root: camino::Utf8PathBuf,
    files: HashMap<String, String>,
}

impl MockRepoView {
    fn new(root: &str) -> Self {
        Self {
            root: camino::Utf8PathBuf::from(root),
            files: HashMap::new(),
        }
    }

    fn with_file(mut self, path: &str, content: &str) -> Self {
        self.files.insert(path.to_string(), content.to_string());
        self
    }
}

impl RepoView for MockRepoView {
    fn root(&self) -> &camino::Utf8Path {
        &self.root
    }

    fn read_to_string(&self, rel: &camino::Utf8Path) -> anyhow::Result<String> {
        self.files
            .get(rel.as_str())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("File not found: {}", rel))
    }

    fn exists(&self, rel: &camino::Utf8Path) -> bool {
        self.files.contains_key(rel.as_str())
    }
}

/// Mock implementation of Fixer for testing the trait contract
struct MockFixer {
    meta: FixerMeta,
    ops: Vec<PlanOp>,
}

impl MockFixer {
    fn new(fix_key: &'static str, safety: SafetyClass) -> Self {
        Self {
            meta: FixerMeta {
                fix_key,
                description: "Mock fixer for testing",
                safety,
                consumes_sensors: &[],
                consumes_check_ids: &[],
            },
            ops: vec![],
        }
    }

    fn with_ops(mut self, ops: Vec<PlanOp>) -> Self {
        self.ops = ops;
        self
    }
}

impl Fixer for MockFixer {
    fn meta(&self) -> FixerMeta {
        self.meta.clone()
    }

    fn plan(
        &self,
        _ctx: &PlanContext,
        _repo: &dyn RepoView,
        _receipts: &ReceiptSet,
    ) -> anyhow::Result<Vec<PlanOp>> {
        Ok(self.ops.clone())
    }
}

// =============================================================================
// Test Helpers
// =============================================================================

fn make_receipt(sensor_id: &str, findings: Vec<Finding>) -> ReceiptEnvelope {
    ReceiptEnvelope {
        schema: "test".to_string(),
        tool: ToolInfo {
            name: sensor_id.to_string(),
            version: None,
            repo: None,
            commit: None,
        },
        run: Default::default(),
        verdict: Default::default(),
        findings,
        capabilities: None,
        data: None,
    }
}

fn make_finding(check_id: &str, code: Option<&str>) -> Finding {
    Finding {
        severity: Severity::Error,
        check_id: Some(check_id.to_string()),
        code: code.map(String::from),
        message: None,
        location: Some(Location {
            path: "Cargo.toml".into(),
            line: Some(1),
            column: None,
        }),
        fingerprint: None,
        data: None,
        confidence: None,
        provenance: None,
        context: None,
    }
}

fn make_plan_op(id: &str, safety: SafetyClass) -> PlanOp {
    PlanOp {
        id: id.to_string(),
        safety,
        blocked: false,
        blocked_reason: None,
        blocked_reason_token: None,
        target: OpTarget {
            path: "Cargo.toml".to_string(),
        },
        kind: OpKind::TomlSet {
            toml_path: vec!["workspace".to_string(), "resolver".to_string()],
            value: serde_json::json!("2"),
        },
        rationale: Rationale {
            fix_key: "test.fix".to_string(),
            description: Some("Test operation".to_string()),
            findings: vec![],
        },
        params_required: vec![],
        preview: None,
    }
}

fn make_finding_ref(source: &str, code: &str) -> FindingRef {
    FindingRef {
        source: source.to_string(),
        check_id: Some("test.check".to_string()),
        code: code.to_string(),
        path: Some("Cargo.toml".to_string()),
        line: Some(1),
        fingerprint: None,
    }
}

// =============================================================================
// FixerMeta Tests
// =============================================================================

#[test]
fn test_fixer_meta_creation() {
    let meta = FixerMeta {
        fix_key: "cargo.workspace_resolver_v2",
        description: "Sets workspace resolver to version 2",
        safety: SafetyClass::Safe,
        consumes_sensors: &["cargo-deny", "cargo-outdated"],
        consumes_check_ids: &["workspace.resolver"],
    };

    assert_eq!(meta.fix_key, "cargo.workspace_resolver_v2");
    assert_eq!(meta.description, "Sets workspace resolver to version 2");
    assert!(meta.safety.is_safe());
    assert_eq!(meta.consumes_sensors.len(), 2);
    assert_eq!(meta.consumes_check_ids.len(), 1);
}

#[test]
fn test_fixer_meta_serialization() {
    let meta = FixerMeta {
        fix_key: "test.fixer",
        description: "Test fixer",
        safety: SafetyClass::Guarded,
        consumes_sensors: &[],
        consumes_check_ids: &[],
    };

    let json = serde_json::to_string(&meta).expect("Should serialize to JSON");
    assert!(json.contains("test.fixer"));
    assert!(json.contains("Test fixer"));
    assert!(json.contains("guarded"));
}

#[test]
fn test_fixer_meta_with_all_safety_classes() {
    let safe_meta = FixerMeta {
        fix_key: "safe.op",
        description: "Safe operation",
        safety: SafetyClass::Safe,
        consumes_sensors: &[],
        consumes_check_ids: &[],
    };
    assert!(safe_meta.safety.is_safe());
    assert!(!safe_meta.safety.is_guarded());
    assert!(!safe_meta.safety.is_unsafe());

    let guarded_meta = FixerMeta {
        fix_key: "guarded.op",
        description: "Guarded operation",
        safety: SafetyClass::Guarded,
        consumes_sensors: &[],
        consumes_check_ids: &[],
    };
    assert!(!guarded_meta.safety.is_safe());
    assert!(guarded_meta.safety.is_guarded());
    assert!(!guarded_meta.safety.is_unsafe());

    let unsafe_meta = FixerMeta {
        fix_key: "unsafe.op",
        description: "Unsafe operation",
        safety: SafetyClass::Unsafe,
        consumes_sensors: &[],
        consumes_check_ids: &[],
    };
    assert!(!unsafe_meta.safety.is_safe());
    assert!(!unsafe_meta.safety.is_guarded());
    assert!(unsafe_meta.safety.is_unsafe());
}

// =============================================================================
// PlannerConfig Tests
// =============================================================================

#[test]
fn test_planner_config_defaults() {
    let config = PlannerConfig::default();

    assert!(config.allow.is_empty());
    assert!(config.deny.is_empty());
    assert!(!config.allow_guarded);
    assert!(!config.allow_unsafe);
    assert!(!config.allow_dirty);
    assert!(config.max_ops.is_none());
    assert!(config.max_files.is_none());
    assert!(config.max_patch_bytes.is_none());
    assert!(config.params.is_empty());
}

#[test]
fn test_planner_config_with_values() {
    let mut params = HashMap::new();
    params.insert("key".to_string(), "value".to_string());

    let config = PlannerConfig {
        allow: vec!["fix1".to_string(), "fix2".to_string()],
        deny: vec!["fix3".to_string()],
        allow_guarded: true,
        allow_unsafe: false,
        allow_dirty: true,
        max_ops: Some(100),
        max_files: Some(10),
        max_patch_bytes: Some(1024),
        params,
    };

    assert_eq!(config.allow.len(), 2);
    assert_eq!(config.deny.len(), 1);
    assert!(config.allow_guarded);
    assert!(!config.allow_unsafe);
    assert!(config.allow_dirty);
    assert_eq!(config.max_ops, Some(100));
    assert_eq!(config.max_files, Some(10));
    assert_eq!(config.max_patch_bytes, Some(1024));
    assert_eq!(config.params.get("key"), Some(&"value".to_string()));
}

#[test]
fn test_planner_config_clone() {
    let config = PlannerConfig {
        allow: vec!["fix1".to_string()],
        deny: vec![],
        allow_guarded: true,
        allow_unsafe: false,
        allow_dirty: false,
        max_ops: Some(50),
        max_files: None,
        max_patch_bytes: None,
        params: HashMap::new(),
    };

    let cloned = config.clone();
    assert_eq!(cloned.allow, config.allow);
    assert_eq!(cloned.allow_guarded, config.allow_guarded);
    assert_eq!(cloned.max_ops, config.max_ops);
}

// =============================================================================
// PlanContext Tests
// =============================================================================

#[test]
fn test_plan_context_creation() {
    let ctx = PlanContext {
        repo_root: camino::Utf8PathBuf::from("/repo"),
        artifacts_dir: camino::Utf8PathBuf::from("/repo/artifacts"),
        config: PlannerConfig::default(),
    };

    assert_eq!(ctx.repo_root.as_str(), "/repo");
    assert_eq!(ctx.artifacts_dir.as_str(), "/repo/artifacts");
}

#[test]
fn test_plan_context_clone() {
    let ctx = PlanContext {
        repo_root: camino::Utf8PathBuf::from("/repo"),
        artifacts_dir: camino::Utf8PathBuf::from("/repo/artifacts"),
        config: PlannerConfig {
            allow: vec!["test".to_string()],
            ..Default::default()
        },
    };

    let cloned = ctx.clone();
    assert_eq!(cloned.repo_root, ctx.repo_root);
    assert_eq!(cloned.artifacts_dir, ctx.artifacts_dir);
    assert_eq!(cloned.config.allow, ctx.config.allow);
}

// =============================================================================
// RepoView Tests
// =============================================================================

#[test]
fn test_mock_repo_view_root() {
    let repo = MockRepoView::new("/test/repo");
    assert_eq!(repo.root().as_str(), "/test/repo");
}

#[test]
fn test_mock_repo_view_file_operations() {
    let repo = MockRepoView::new("/repo")
        .with_file("Cargo.toml", "[package]\nname = \"test\"")
        .with_file("src/lib.rs", "pub fn test() {}");

    let cargo_path = Utf8Path::new("Cargo.toml");
    let lib_path = Utf8Path::new("src/lib.rs");
    let missing_path = Utf8Path::new("missing.txt");

    assert!(repo.exists(cargo_path));
    assert!(repo.exists(lib_path));
    assert!(!repo.exists(missing_path));

    let content = repo
        .read_to_string(cargo_path)
        .expect("Should read Cargo.toml");
    assert!(content.contains("[package]"));

    let result = repo.read_to_string(missing_path);
    assert!(result.is_err());
}

#[test]
fn test_mock_repo_view_empty() {
    let repo = MockRepoView::new("/empty");
    let path = Utf8Path::new("any.txt");

    assert!(!repo.exists(path));
    assert!(repo.read_to_string(path).is_err());
}

// =============================================================================
// Fixer Trait Tests
// =============================================================================

#[test]
fn test_fixer_trait_meta() {
    let fixer = MockFixer::new("test.fixer", SafetyClass::Safe);
    let meta = fixer.meta();

    assert_eq!(meta.fix_key, "test.fixer");
    assert!(meta.safety.is_safe());
}

#[test]
fn test_fixer_trait_plan_returns_ops() {
    let ops = vec![
        make_plan_op("op1", SafetyClass::Safe),
        make_plan_op("op2", SafetyClass::Guarded),
    ];

    let fixer = MockFixer::new("test.fixer", SafetyClass::Safe).with_ops(ops.clone());

    let ctx = PlanContext {
        repo_root: camino::Utf8PathBuf::from("/repo"),
        artifacts_dir: camino::Utf8PathBuf::from("/repo/artifacts"),
        config: PlannerConfig::default(),
    };
    let repo = MockRepoView::new("/repo");
    let receipts = ReceiptSet::from_loaded(&[]);

    let result = fixer
        .plan(&ctx, &repo, &receipts)
        .expect("Plan should succeed");

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, "op1");
    assert_eq!(result[1].id, "op2");
}

#[test]
fn test_fixer_trait_plan_empty_result() {
    let fixer = MockFixer::new("empty.fixer", SafetyClass::Safe);

    let ctx = PlanContext {
        repo_root: camino::Utf8PathBuf::from("/repo"),
        artifacts_dir: camino::Utf8PathBuf::from("/repo/artifacts"),
        config: PlannerConfig::default(),
    };
    let repo = MockRepoView::new("/repo");
    let receipts = ReceiptSet::from_loaded(&[]);

    let result = fixer
        .plan(&ctx, &repo, &receipts)
        .expect("Plan should succeed");
    assert!(result.is_empty());
}

// =============================================================================
// MatchedFinding Tests
// =============================================================================

#[test]
fn test_matched_finding_creation() {
    let finding = Finding {
        severity: Severity::Warn,
        check_id: Some("test.check".to_string()),
        code: Some("E001".to_string()),
        message: Some("Test message".to_string()),
        location: Some(Location {
            path: "src/main.rs".into(),
            line: Some(42),
            column: Some(10),
        }),
        fingerprint: Some("abc123".to_string()),
        data: Some(serde_json::json!({"extra": "data"})),
        confidence: Some(0.95),
        provenance: None,
        context: None,
    };

    let receipt = make_receipt("test-tool", vec![finding]);
    let loaded = vec![LoadedReceipt {
        path: "artifacts/test-tool/report.json".into(),
        sensor_id: "test-tool".to_string(),
        receipt: Ok(receipt),
    }];
    let set = ReceiptSet::from_loaded(&loaded);

    let matches = set.matching_findings_with_data(&["test-tool"], &["test.check"], &["E001"]);
    assert_eq!(matches.len(), 1);

    let matched = &matches[0];
    assert_eq!(matched.finding.source, "test-tool");
    assert_eq!(matched.finding.check_id, Some("test.check".to_string()));
    assert_eq!(matched.finding.code, "E001");
    assert_eq!(matched.finding.path, Some("src/main.rs".to_string()));
    assert_eq!(matched.finding.line, Some(42));
    assert_eq!(matched.finding.fingerprint, Some("abc123".to_string()));
    assert_eq!(matched.confidence, Some(0.95));
    assert!(matched.data.is_some());
}

#[test]
fn test_matched_finding_minimal() {
    let finding = Finding {
        severity: Severity::Info,
        check_id: None,
        code: None,
        message: None,
        location: None,
        fingerprint: None,
        data: None,
        confidence: None,
        provenance: None,
        context: None,
    };

    let receipt = make_receipt("minimal-tool", vec![finding]);
    let loaded = vec![LoadedReceipt {
        path: "artifacts/minimal-tool/report.json".into(),
        sensor_id: "minimal-tool".to_string(),
        receipt: Ok(receipt),
    }];
    let set = ReceiptSet::from_loaded(&loaded);

    // Empty filters match all
    let matches = set.matching_findings(&["minimal-tool"], &[], &[]);
    assert_eq!(matches.len(), 1);

    let matched = &matches[0];
    assert_eq!(matched.source, "minimal-tool");
    assert_eq!(matched.check_id, None);
    assert_eq!(matched.code, "-"); // Default code when None
    assert_eq!(matched.path, None);
    assert_eq!(matched.line, None);
}

#[test]
fn test_matched_finding_clone() {
    let original = MatchedFinding {
        finding: make_finding_ref("tool", "CODE"),
        data: Some(serde_json::json!({"key": "value"})),
        confidence: Some(0.8),
        provenance: None,
        context: None,
    };

    let cloned = original.clone();
    assert_eq!(cloned.finding.source, original.finding.source);
    assert_eq!(cloned.confidence, original.confidence);
}

// =============================================================================
// ReceiptSet Additional Tests
// =============================================================================

#[test]
fn test_receipt_set_empty() {
    let set = ReceiptSet::from_loaded(&[]);

    let matches = set.matching_findings(&["any"], &[], &[]);
    assert!(matches.is_empty());
}

#[test]
fn test_receipt_set_multiple_receipts() {
    let receipt1 = make_receipt("tool-a", vec![make_finding("check.a", Some("code1"))]);
    let receipt2 = make_receipt("tool-b", vec![make_finding("check.b", Some("code2"))]);

    let loaded = vec![
        LoadedReceipt {
            path: "artifacts/tool-a/report.json".into(),
            sensor_id: "tool-a".to_string(),
            receipt: Ok(receipt1),
        },
        LoadedReceipt {
            path: "artifacts/tool-b/report.json".into(),
            sensor_id: "tool-b".to_string(),
            receipt: Ok(receipt2),
        },
    ];
    let set = ReceiptSet::from_loaded(&loaded);

    let matches = set.matching_findings(&["tool-a", "tool-b"], &[], &[]);
    assert_eq!(matches.len(), 2);
}

#[test]
fn test_receipt_set_tool_prefix_matching() {
    let receipt = make_receipt("cargo-deny", vec![make_finding("check", None)]);
    let loaded = vec![LoadedReceipt {
        path: "artifacts/cargo-deny/report.json".into(),
        sensor_id: "cargo-deny".to_string(),
        receipt: Ok(receipt),
    }];
    let set = ReceiptSet::from_loaded(&loaded);

    // Prefix "cargo" should match "cargo-deny"
    let matches = set.matching_findings(&["cargo"], &[], &[]);
    assert_eq!(matches.len(), 1);

    // Prefix "cargo-deny" should match exactly
    let matches = set.matching_findings(&["cargo-deny"], &[], &[]);
    assert_eq!(matches.len(), 1);

    // Prefix "other" should not match
    let matches = set.matching_findings(&["other"], &[], &[]);
    assert!(matches.is_empty());
}

#[test]
fn test_receipt_set_sorted_output() {
    let receipt = make_receipt(
        "tool",
        vec![
            make_finding("check.z", None),
            make_finding("check.a", None),
            make_finding("check.m", None),
        ],
    );
    let loaded = vec![LoadedReceipt {
        path: "artifacts/tool/report.json".into(),
        sensor_id: "tool".to_string(),
        receipt: Ok(receipt),
    }];
    let set = ReceiptSet::from_loaded(&loaded);

    let matches = set.matching_findings(&["tool"], &[], &[]);
    assert_eq!(matches.len(), 3);

    // Results should be sorted by stable_finding_key
    let codes: Vec<&str> = matches.iter().map(|m| m.code.as_str()).collect();
    // The sorting is by source/check_id/code|location
    assert!(codes.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn test_receipt_set_multiple_findings_single_receipt() {
    let receipt = make_receipt(
        "multi-tool",
        vec![
            make_finding("check.1", Some("code1")),
            make_finding("check.2", Some("code2")),
            make_finding("check.3", Some("code3")),
        ],
    );
    let loaded = vec![LoadedReceipt {
        path: "artifacts/multi-tool/report.json".into(),
        sensor_id: "multi-tool".to_string(),
        receipt: Ok(receipt),
    }];
    let set = ReceiptSet::from_loaded(&loaded);

    let matches = set.matching_findings(&["multi-tool"], &[], &[]);
    assert_eq!(matches.len(), 3);
}

#[test]
fn test_receipt_set_check_id_filtering() {
    let receipt = make_receipt(
        "tool",
        vec![
            make_finding("wanted.check", Some("code")),
            make_finding("unwanted.check", Some("code")),
        ],
    );
    let loaded = vec![LoadedReceipt {
        path: "artifacts/tool/report.json".into(),
        sensor_id: "tool".to_string(),
        receipt: Ok(receipt),
    }];
    let set = ReceiptSet::from_loaded(&loaded);

    let matches = set.matching_findings(&["tool"], &["wanted.check"], &[]);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].check_id, Some("wanted.check".to_string()));
}

#[test]
fn test_receipt_set_code_filtering() {
    let receipt = make_receipt(
        "tool",
        vec![
            make_finding("check", Some("wanted_code")),
            make_finding("check", Some("unwanted_code")),
        ],
    );
    let loaded = vec![LoadedReceipt {
        path: "artifacts/tool/report.json".into(),
        sensor_id: "tool".to_string(),
        receipt: Ok(receipt),
    }];
    let set = ReceiptSet::from_loaded(&loaded);

    let matches = set.matching_findings(&["tool"], &["check"], &["wanted_code"]);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].code, "wanted_code");
}

#[test]
fn test_receipt_set_location_handling() {
    let finding_with_location = Finding {
        severity: Severity::Error,
        check_id: Some("check".to_string()),
        code: Some("code".to_string()),
        message: None,
        location: Some(Location {
            path: "src/deep/file.rs".into(),
            line: Some(100),
            column: Some(50),
        }),
        fingerprint: None,
        data: None,
        confidence: None,
        provenance: None,
        context: None,
    };

    let receipt = make_receipt("tool", vec![finding_with_location]);
    let loaded = vec![LoadedReceipt {
        path: "artifacts/tool/report.json".into(),
        sensor_id: "tool".to_string(),
        receipt: Ok(receipt),
    }];
    let set = ReceiptSet::from_loaded(&loaded);

    let matches = set.matching_findings(&["tool"], &["check"], &["code"]);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].path, Some("src/deep/file.rs".to_string()));
    assert_eq!(matches[0].line, Some(100));
}

// =============================================================================
// SafetyClass Integration Tests
// =============================================================================

#[test]
fn test_safety_class_safe_predicate() {
    assert!(SafetyClass::Safe.is_safe());
    assert!(!SafetyClass::Guarded.is_safe());
    assert!(!SafetyClass::Unsafe.is_safe());
}

#[test]
fn test_safety_class_guarded_predicate() {
    assert!(!SafetyClass::Safe.is_guarded());
    assert!(SafetyClass::Guarded.is_guarded());
    assert!(!SafetyClass::Unsafe.is_guarded());
}

#[test]
fn test_safety_class_unsafe_predicate() {
    assert!(!SafetyClass::Safe.is_unsafe());
    assert!(!SafetyClass::Guarded.is_unsafe());
    assert!(SafetyClass::Unsafe.is_unsafe());
}

#[test]
fn test_safety_class_serialization() {
    let safe = SafetyClass::Safe;
    let guarded = SafetyClass::Guarded;
    let unsafe_val = SafetyClass::Unsafe;

    assert_eq!(serde_json::to_string(&safe).unwrap(), "\"safe\"");
    assert_eq!(serde_json::to_string(&guarded).unwrap(), "\"guarded\"");
    assert_eq!(serde_json::to_string(&unsafe_val).unwrap(), "\"unsafe\"");
}

#[test]
fn test_safety_class_deserialization() {
    let safe: SafetyClass = serde_json::from_str("\"safe\"").unwrap();
    let guarded: SafetyClass = serde_json::from_str("\"guarded\"").unwrap();
    let unsafe_val: SafetyClass = serde_json::from_str("\"unsafe\"").unwrap();

    assert!(safe.is_safe());
    assert!(guarded.is_guarded());
    assert!(unsafe_val.is_unsafe());
}

// =============================================================================
// PlanOp Integration Tests
// =============================================================================

#[test]
fn test_plan_op_creation() {
    let op = make_plan_op("test-op-1", SafetyClass::Guarded);

    assert_eq!(op.id, "test-op-1");
    assert!(op.safety.is_guarded());
    assert!(!op.blocked);
    assert!(op.blocked_reason.is_none());
    assert!(op.blocked_reason_token.is_none());
    assert_eq!(op.target.path, "Cargo.toml");
}

#[test]
fn test_plan_op_with_blocked_status() {
    let mut op = make_plan_op("blocked-op", SafetyClass::Guarded);
    op.blocked = true;
    op.blocked_reason = Some("Not in allow list".to_string());
    op.blocked_reason_token = Some("allowlist_missing".to_string());

    assert!(op.blocked);
    assert_eq!(op.blocked_reason, Some("Not in allow list".to_string()));
    assert_eq!(
        op.blocked_reason_token,
        Some("allowlist_missing".to_string())
    );
}

#[test]
fn test_plan_op_with_preview() {
    let mut op = make_plan_op("preview-op", SafetyClass::Safe);
    op.preview = Some(OpPreview {
        patch_fragment: "@@ -1,1 +1,1 @@\n-resolver = \"1\"\n+resolver = \"2\"\n".to_string(),
    });

    assert!(op.preview.is_some());
    let preview = op.preview.unwrap();
    assert!(preview.patch_fragment.contains("resolver"));
}

#[test]
fn test_plan_op_with_params_required() {
    let mut op = make_plan_op("params-op", SafetyClass::Unsafe);
    op.params_required = vec!["version".to_string(), "edition".to_string()];

    assert_eq!(op.params_required.len(), 2);
    assert!(op.params_required.contains(&"version".to_string()));
    assert!(op.params_required.contains(&"edition".to_string()));
}

#[test]
fn test_plan_op_rationale() {
    let mut op = make_plan_op("rationale-op", SafetyClass::Safe);
    op.rationale = Rationale {
        fix_key: "cargo.workspace_resolver_v2".to_string(),
        description: Some(
            "Updates workspace resolver to v2 for improved dependency resolution".to_string(),
        ),
        findings: vec![
            make_finding_ref("cargo-deny", "RESOLVER_V1"),
            make_finding_ref("cargo-outdated", "OLD_RESOLVER"),
        ],
    };

    assert_eq!(op.rationale.fix_key, "cargo.workspace_resolver_v2");
    assert!(op.rationale.description.is_some());
    assert_eq!(op.rationale.findings.len(), 2);
}

// =============================================================================
// OpKind Tests
// =============================================================================

#[test]
fn test_op_kind_toml_set() {
    let kind = OpKind::TomlSet {
        toml_path: vec![
            "workspace".to_string(),
            "package".to_string(),
            "version".to_string(),
        ],
        value: serde_json::json!("1.0.0"),
    };

    let json = serde_json::to_string(&kind).expect("Should serialize");
    assert!(json.contains("toml_set"));
    assert!(json.contains("workspace"));
}

#[test]
fn test_op_kind_toml_remove() {
    let kind = OpKind::TomlRemove {
        toml_path: vec!["dependencies".to_string(), "unused".to_string()],
    };

    let json = serde_json::to_string(&kind).expect("Should serialize");
    assert!(json.contains("toml_remove"));
}

#[test]
fn test_op_kind_toml_transform() {
    let kind = OpKind::TomlTransform {
        rule_id: "sort_dependencies".to_string(),
        args: Some(serde_json::json!({"group": true})),
    };

    let json = serde_json::to_string(&kind).expect("Should serialize");
    assert!(json.contains("toml_transform"));
    assert!(json.contains("sort_dependencies"));
}

#[test]
fn test_op_kind_text_replace_anchored() {
    let kind = OpKind::TextReplaceAnchored {
        find: "old_text".to_string(),
        replace: "new_text".to_string(),
        anchor_before: vec!["// START".to_string()],
        anchor_after: vec!["// END".to_string()],
        max_replacements: Some(1),
    };

    let json = serde_json::to_string(&kind).expect("Should serialize");
    assert!(json.contains("text_replace_anchored"));
}

// =============================================================================
// FindingRef Tests
// =============================================================================

#[test]
fn test_finding_ref_complete() {
    let finding = FindingRef {
        source: "cargo-clippy".to_string(),
        check_id: Some("clippy::unwrap_used".to_string()),
        code: "UNWRAP_USED".to_string(),
        path: Some("src/lib.rs".to_string()),
        line: Some(42),
        fingerprint: Some("hash123".to_string()),
    };

    assert_eq!(finding.source, "cargo-clippy");
    assert_eq!(finding.check_id, Some("clippy::unwrap_used".to_string()));
    assert_eq!(finding.code, "UNWRAP_USED");
    assert_eq!(finding.path, Some("src/lib.rs".to_string()));
    assert_eq!(finding.line, Some(42));
    assert_eq!(finding.fingerprint, Some("hash123".to_string()));
}

#[test]
fn test_finding_ref_minimal() {
    let finding = FindingRef {
        source: "tool".to_string(),
        check_id: None,
        code: "CODE".to_string(),
        path: None,
        line: None,
        fingerprint: None,
    };

    assert_eq!(finding.source, "tool");
    assert!(finding.check_id.is_none());
    assert!(finding.path.is_none());
    assert!(finding.line.is_none());
    assert!(finding.fingerprint.is_none());
}

#[test]
fn test_finding_ref_serialization() {
    let finding = FindingRef {
        source: "test".to_string(),
        check_id: Some("check.id".to_string()),
        code: "CODE".to_string(),
        path: Some("file.rs".to_string()),
        line: Some(10),
        fingerprint: None,
    };

    let json = serde_json::to_string(&finding).expect("Should serialize");
    assert!(json.contains("test"));
    assert!(json.contains("check.id"));
    assert!(json.contains("CODE"));
    assert!(json.contains("file.rs"));
}

// =============================================================================
// Edge Cases and Error Conditions
// =============================================================================

#[test]
fn test_empty_tool_prefixes_matches_none() {
    let receipt = make_receipt("tool", vec![make_finding("check", None)]);
    let loaded = vec![LoadedReceipt {
        path: "artifacts/tool/report.json".into(),
        sensor_id: "tool".to_string(),
        receipt: Ok(receipt),
    }];
    let set = ReceiptSet::from_loaded(&loaded);

    // Empty tool_prefixes should not match any tool
    let matches = set.matching_findings(&[], &[], &[]);
    assert!(matches.is_empty());
}

#[test]
fn test_receipt_set_with_all_error_receipts() {
    let loaded = vec![
        LoadedReceipt {
            path: "artifacts/tool1/report.json".into(),
            sensor_id: "tool1".to_string(),
            receipt: Err(buildfix_receipts::ReceiptLoadError::Io {
                message: "not found".to_string(),
            }),
        },
        LoadedReceipt {
            path: "artifacts/tool2/report.json".into(),
            sensor_id: "tool2".to_string(),
            receipt: Err(buildfix_receipts::ReceiptLoadError::Json {
                message: "invalid json".to_string(),
            }),
        },
    ];
    let set = ReceiptSet::from_loaded(&loaded);

    // Should have no valid receipts
    let matches = set.matching_findings(&["tool1", "tool2"], &[], &[]);
    assert!(matches.is_empty());
}

#[test]
fn test_finding_with_no_code_defaults_to_dash() {
    let finding = Finding {
        severity: Severity::Error,
        check_id: Some("check".to_string()),
        code: None,
        message: None,
        location: None,
        fingerprint: None,
        data: None,
        confidence: None,
        provenance: None,
        context: None,
    };

    let receipt = make_receipt("tool", vec![finding]);
    let loaded = vec![LoadedReceipt {
        path: "artifacts/tool/report.json".into(),
        sensor_id: "tool".to_string(),
        receipt: Ok(receipt),
    }];
    let set = ReceiptSet::from_loaded(&loaded);

    let matches = set.matching_findings(&["tool"], &[], &[]);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].code, "-");
}

#[test]
fn test_finding_with_special_characters_in_code() {
    let finding = Finding {
        severity: Severity::Error,
        check_id: Some("check.id.with.dots".to_string()),
        code: Some("code-with-special_chars.123".to_string()),
        message: None,
        location: Some(Location {
            path: "path/with spaces/file.rs".into(),
            line: Some(1),
            column: None,
        }),
        fingerprint: None,
        data: None,
        confidence: None,
        provenance: None,
        context: None,
    };

    let receipt = make_receipt("tool", vec![finding]);
    let loaded = vec![LoadedReceipt {
        path: "artifacts/tool/report.json".into(),
        sensor_id: "tool".to_string(),
        receipt: Ok(receipt),
    }];
    let set = ReceiptSet::from_loaded(&loaded);

    let matches = set.matching_findings(
        &["tool"],
        &["check.id.with.dots"],
        &["code-with-special_chars.123"],
    );
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].code, "code-with-special_chars.123");
}
