//! Integration tests for buildfix-core-runtime.
//!
//! Tests runtime adapters, settings, and ports implementation.

use buildfix_core_runtime::ports::{GitPort, ReceiptSource, WritePort};
use buildfix_core_runtime::settings::{ApplySettings, PlanSettings, RunMode};
use buildfix_receipts::{LoadedReceipt, ReceiptLoadError};
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::HashMap;
use tempfile::TempDir;

// ============================================================================
// RunMode Tests
// ============================================================================

#[test]
fn test_run_mode_default() {
    let mode = RunMode::default();
    assert!(matches!(mode, RunMode::Standalone));
}

#[test]
fn test_run_mode_variants() {
    let standalone = RunMode::Standalone;
    let cockpit = RunMode::Cockpit;

    // Ensure variants exist and can be compared
    assert_ne!(standalone, cockpit);
    assert_eq!(standalone, RunMode::Standalone);
    assert_eq!(cockpit, RunMode::Cockpit);
}

// ============================================================================
// PlanSettings Tests
// ============================================================================

#[test]
fn test_plan_settings_default() {
    let settings = PlanSettings::default();

    assert_eq!(settings.repo_root, Utf8PathBuf::from("."));
    assert_eq!(settings.artifacts_dir, Utf8PathBuf::from("artifacts"));
    assert_eq!(settings.out_dir, Utf8PathBuf::from("artifacts/buildfix"));
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
    assert!(matches!(settings.mode, RunMode::Standalone));
}

#[test]
fn test_plan_settings_custom() {
    let mut params = HashMap::new();
    params.insert("key1".to_string(), "value1".to_string());

    let settings = PlanSettings {
        repo_root: Utf8PathBuf::from("/custom/repo"),
        artifacts_dir: Utf8PathBuf::from("/custom/artifacts"),
        out_dir: Utf8PathBuf::from("/custom/out"),
        allow: vec!["fix1".to_string(), "fix2".to_string()],
        deny: vec!["fix3".to_string()],
        allow_guarded: true,
        allow_unsafe: true,
        allow_dirty: true,
        max_ops: Some(100),
        max_files: Some(50),
        max_patch_bytes: Some(10000),
        params,
        require_clean_hashes: false,
        git_head_precondition: true,
        backup_suffix: ".bak".to_string(),
        mode: RunMode::Cockpit,
    };

    assert_eq!(settings.repo_root, Utf8PathBuf::from("/custom/repo"));
    assert_eq!(
        settings.artifacts_dir,
        Utf8PathBuf::from("/custom/artifacts")
    );
    assert_eq!(settings.out_dir, Utf8PathBuf::from("/custom/out"));
    assert_eq!(settings.allow.len(), 2);
    assert_eq!(settings.deny.len(), 1);
    assert!(settings.allow_guarded);
    assert!(settings.allow_unsafe);
    assert!(settings.allow_dirty);
    assert_eq!(settings.max_ops, Some(100));
    assert_eq!(settings.max_files, Some(50));
    assert_eq!(settings.max_patch_bytes, Some(10000));
    assert_eq!(settings.params.get("key1"), Some(&"value1".to_string()));
    assert!(!settings.require_clean_hashes);
    assert!(settings.git_head_precondition);
    assert_eq!(settings.backup_suffix, ".bak");
    assert!(matches!(settings.mode, RunMode::Cockpit));
}

#[test]
fn test_plan_settings_clone() {
    let settings = PlanSettings {
        repo_root: Utf8PathBuf::from("/repo"),
        ..Default::default()
    };

    let cloned = settings.clone();
    assert_eq!(settings.repo_root, cloned.repo_root);
}

// ============================================================================
// ApplySettings Tests
// ============================================================================

#[test]
fn test_apply_settings_default() {
    let settings = ApplySettings::default();

    assert_eq!(settings.repo_root, Utf8PathBuf::from("."));
    assert_eq!(settings.out_dir, Utf8PathBuf::from("artifacts/buildfix"));
    assert!(settings.dry_run);
    assert!(!settings.allow_guarded);
    assert!(!settings.allow_unsafe);
    assert!(!settings.allow_dirty);
    assert!(settings.params.is_empty());
    assert!(!settings.auto_commit);
    assert!(settings.commit_message.is_none());
    assert!(settings.backup_enabled);
    assert_eq!(settings.backup_suffix, ".buildfix.bak");
    assert!(matches!(settings.mode, RunMode::Standalone));
}

#[test]
fn test_apply_settings_custom() {
    let mut params = HashMap::new();
    params.insert("param1".to_string(), "value1".to_string());

    let settings = ApplySettings {
        repo_root: Utf8PathBuf::from("/custom/repo"),
        out_dir: Utf8PathBuf::from("/custom/out"),
        dry_run: false,
        allow_guarded: true,
        allow_unsafe: true,
        allow_dirty: true,
        params,
        auto_commit: true,
        commit_message: Some("Auto-fix commit".to_string()),
        backup_enabled: false,
        backup_suffix: ".backup".to_string(),
        mode: RunMode::Cockpit,
    };

    assert_eq!(settings.repo_root, Utf8PathBuf::from("/custom/repo"));
    assert_eq!(settings.out_dir, Utf8PathBuf::from("/custom/out"));
    assert!(!settings.dry_run);
    assert!(settings.allow_guarded);
    assert!(settings.allow_unsafe);
    assert!(settings.allow_dirty);
    assert_eq!(settings.params.get("param1"), Some(&"value1".to_string()));
    assert!(settings.auto_commit);
    assert_eq!(settings.commit_message, Some("Auto-fix commit".to_string()));
    assert!(!settings.backup_enabled);
    assert_eq!(settings.backup_suffix, ".backup");
    assert!(matches!(settings.mode, RunMode::Cockpit));
}

#[test]
fn test_apply_settings_clone() {
    let settings = ApplySettings {
        repo_root: Utf8PathBuf::from("/repo"),
        ..Default::default()
    };

    let cloned = settings.clone();
    assert_eq!(settings.repo_root, cloned.repo_root);
}

// ============================================================================
// Port Trait Tests - Mock Implementations
// ============================================================================

/// Mock ReceiptSource for testing
struct MockReceiptSource {
    receipts: Vec<LoadedReceipt>,
}

impl MockReceiptSource {
    fn new(receipts: Vec<LoadedReceipt>) -> Self {
        Self { receipts }
    }

    fn empty() -> Self {
        Self {
            receipts: Vec::new(),
        }
    }
}

impl ReceiptSource for MockReceiptSource {
    fn load_receipts(&self) -> anyhow::Result<Vec<LoadedReceipt>> {
        Ok(self.receipts.clone())
    }
}

/// Mock WritePort for testing
#[derive(Debug, Default)]
struct MockWritePort {
    written_files: std::cell::RefCell<Vec<(String, Vec<u8>)>>,
    created_dirs: std::cell::RefCell<Vec<String>>,
}

impl MockWritePort {
    fn new() -> Self {
        Self::default()
    }

    fn written_files(&self) -> Vec<(String, Vec<u8>)> {
        self.written_files.borrow().clone()
    }

    fn created_dirs(&self) -> Vec<String> {
        self.created_dirs.borrow().clone()
    }
}

impl WritePort for MockWritePort {
    fn write_file(&self, path: &Utf8Path, contents: &[u8]) -> anyhow::Result<()> {
        self.written_files
            .borrow_mut()
            .push((path.to_string(), contents.to_vec()));
        Ok(())
    }

    fn create_dir_all(&self, path: &Utf8Path) -> anyhow::Result<()> {
        self.created_dirs.borrow_mut().push(path.to_string());
        Ok(())
    }
}

/// Mock GitPort for testing
#[derive(Debug, Default)]
struct MockGitPort {
    head_sha: Option<String>,
    is_dirty: Option<bool>,
}

impl MockGitPort {
    fn new() -> Self {
        Self::default()
    }

    fn with_head_sha(mut self, sha: impl Into<String>) -> Self {
        self.head_sha = Some(sha.into());
        self
    }

    fn with_dirty(mut self, dirty: bool) -> Self {
        self.is_dirty = Some(dirty);
        self
    }
}

impl GitPort for MockGitPort {
    fn head_sha(&self, _repo_root: &Utf8Path) -> anyhow::Result<Option<String>> {
        Ok(self.head_sha.clone())
    }

    fn is_dirty(&self, _repo_root: &Utf8Path) -> anyhow::Result<Option<bool>> {
        Ok(self.is_dirty)
    }

    fn commit_all(&self, _repo_root: &Utf8Path, _message: &str) -> anyhow::Result<Option<String>> {
        // Mock doesn't actually commit
        Ok(self.head_sha.clone())
    }
}

// ============================================================================
// ReceiptSource Tests
// ============================================================================

#[test]
fn test_mock_receipt_source_empty() {
    let source = MockReceiptSource::empty();
    let receipts = source.load_receipts().unwrap();
    assert!(receipts.is_empty());
}

#[test]
fn test_mock_receipt_source_with_receipts() {
    let receipt = LoadedReceipt {
        path: Utf8PathBuf::from("artifacts/test/report.json"),
        sensor_id: "test-sensor".to_string(),
        receipt: Err(ReceiptLoadError::Io {
            message: "stub".to_string(),
        }),
    };

    let source = MockReceiptSource::new(vec![receipt]);
    let receipts = source.load_receipts().unwrap();

    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].sensor_id, "test-sensor");
    assert_eq!(
        receipts[0].path,
        Utf8PathBuf::from("artifacts/test/report.json")
    );
}

// ============================================================================
// WritePort Tests
// ============================================================================

#[test]
fn test_mock_write_port_write_file() {
    let port = MockWritePort::new();
    let path = Utf8Path::new("output/test.txt");

    port.write_file(path, b"test content").unwrap();

    let written = port.written_files();
    assert_eq!(written.len(), 1);
    assert_eq!(written[0].0, "output/test.txt");
    assert_eq!(written[0].1, b"test content");
}

#[test]
fn test_mock_write_port_create_dir_all() {
    let port = MockWritePort::new();
    let path = Utf8Path::new("output/nested/dir");

    port.create_dir_all(path).unwrap();

    let dirs = port.created_dirs();
    assert_eq!(dirs.len(), 1);
    assert_eq!(dirs[0], "output/nested/dir");
}

// ============================================================================
// GitPort Tests
// ============================================================================

#[test]
fn test_mock_git_port_head_sha() {
    let port = MockGitPort::new().with_head_sha("abc123def456");

    let sha = port.head_sha(Utf8Path::new(".")).unwrap();
    assert_eq!(sha, Some("abc123def456".to_string()));
}

#[test]
fn test_mock_git_port_head_sha_none() {
    let port = MockGitPort::new();

    let sha = port.head_sha(Utf8Path::new(".")).unwrap();
    assert!(sha.is_none());
}

#[test]
fn test_mock_git_port_is_dirty() {
    let port_dirty = MockGitPort::new().with_dirty(true);
    let port_clean = MockGitPort::new().with_dirty(false);

    let dirty = port_dirty.is_dirty(Utf8Path::new(".")).unwrap();
    let clean = port_clean.is_dirty(Utf8Path::new(".")).unwrap();

    assert_eq!(dirty, Some(true));
    assert_eq!(clean, Some(false));
}

#[test]
fn test_mock_git_port_commit_all() {
    let port = MockGitPort::new().with_head_sha("new-commit-sha");

    let result = port
        .commit_all(Utf8Path::new("."), "Test commit message")
        .unwrap();

    assert_eq!(result, Some("new-commit-sha".to_string()));
}

// ============================================================================
// FsReceiptSource Tests (when fs feature is enabled)
// ============================================================================

#[cfg(feature = "fs")]
mod fs_tests {
    use super::*;
    use buildfix_core_runtime::adapters::FsReceiptSource;

    #[test]
    fn test_fs_receipt_source_new() {
        let source = FsReceiptSource::new(Utf8PathBuf::from("artifacts"));
        assert_eq!(source.artifacts_dir, Utf8PathBuf::from("artifacts"));
    }

    #[test]
    fn test_fs_receipt_source_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let artifacts_dir = Utf8Path::from_path(temp_dir.path()).unwrap();

        let source = FsReceiptSource::new(artifacts_dir.to_owned());
        let receipts = source.load_receipts().unwrap();

        // Empty directory should return empty receipts
        assert!(receipts.is_empty());
    }
}

// ============================================================================
// FsWritePort Tests (when fs feature is enabled)
// ============================================================================

#[cfg(feature = "fs")]
mod fs_write_tests {
    use super::*;
    use buildfix_core_runtime::adapters::FsWritePort;
    use std::fs;

    #[test]
    fn test_fs_write_port_write_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = Utf8Path::from_path(temp_dir.path())
            .unwrap()
            .join("test.txt");

        let port = FsWritePort::default();
        port.write_file(&file_path, b"test content").unwrap();

        let content = fs::read(&file_path).unwrap();
        assert_eq!(content, b"test content");
    }

    #[test]
    fn test_fs_write_port_write_file_creates_parent_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = Utf8Path::from_path(temp_dir.path())
            .unwrap()
            .join("nested/dir/test.txt");

        let port = FsWritePort::default();
        port.write_file(&file_path, b"test content").unwrap();

        let content = fs::read(&file_path).unwrap();
        assert_eq!(content, b"test content");
    }

    #[test]
    fn test_fs_write_port_create_dir_all() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = Utf8Path::from_path(temp_dir.path())
            .unwrap()
            .join("nested/dirs");

        let port = FsWritePort::default();
        port.create_dir_all(&dir_path).unwrap();

        assert!(dir_path.exists());
        assert!(dir_path.is_dir());
    }
}

// ============================================================================
// InMemoryReceiptSource Tests (when memory feature is enabled)
// ============================================================================

#[cfg(feature = "memory")]
mod memory_tests {
    use super::*;
    use buildfix_core_runtime::adapters::InMemoryReceiptSource;

    fn make_receipt(path: &str, sensor_id: &str) -> LoadedReceipt {
        LoadedReceipt {
            path: Utf8PathBuf::from(path),
            sensor_id: sensor_id.to_string(),
            receipt: Err(ReceiptLoadError::Io {
                message: "stub".to_string(),
            }),
        }
    }

    #[test]
    fn test_in_memory_receipt_source_new() {
        let receipts = vec![
            make_receipt("artifacts/sensor1/report.json", "sensor1"),
            make_receipt("artifacts/sensor2/report.json", "sensor2"),
        ];

        let source = InMemoryReceiptSource::new(receipts);
        let loaded = source.load_receipts().unwrap();

        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn test_in_memory_filters_buildfix_receipts() {
        let receipts = vec![
            make_receipt("artifacts/sensor1/report.json", "sensor1"),
            make_receipt("artifacts/buildfix/report.json", "buildfix"),
        ];

        let source = InMemoryReceiptSource::new(receipts);
        let loaded = source.load_receipts().unwrap();

        // buildfix receipt should be filtered out
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].sensor_id, "sensor1");
    }

    #[test]
    fn test_in_memory_filters_cockpit_receipts() {
        let receipts = vec![
            make_receipt("artifacts/sensor1/report.json", "sensor1"),
            make_receipt("artifacts/cockpit/report.json", "cockpit"),
        ];

        let source = InMemoryReceiptSource::new(receipts);
        let loaded = source.load_receipts().unwrap();

        // cockpit receipt should be filtered out
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].sensor_id, "sensor1");
    }

    #[test]
    fn test_in_memory_sorts_by_path() {
        let receipts = vec![
            make_receipt("artifacts/z-sensor/report.json", "z-sensor"),
            make_receipt("artifacts/a-sensor/report.json", "a-sensor"),
            make_receipt("artifacts/m-sensor/report.json", "m-sensor"),
        ];

        let source = InMemoryReceiptSource::new(receipts);
        let loaded = source.load_receipts().unwrap();

        // Should be sorted by path
        assert_eq!(loaded[0].sensor_id, "a-sensor");
        assert_eq!(loaded[1].sensor_id, "m-sensor");
        assert_eq!(loaded[2].sensor_id, "z-sensor");
    }

    #[test]
    fn test_in_memory_empty() {
        let source = InMemoryReceiptSource::new(Vec::new());
        let loaded = source.load_receipts().unwrap();

        assert!(loaded.is_empty());
    }
}

// ============================================================================
// ShellGitPort Tests (when git feature is enabled)
// ============================================================================

#[cfg(feature = "git")]
mod git_shell_tests {
    use buildfix_core_runtime::adapters::ShellGitPort;

    #[test]
    fn test_shell_git_port_default() {
        let port = ShellGitPort::default();
        // Just ensure it can be created
        let _ = port;
    }

    // Note: Testing actual git operations requires a real git repository
    // These tests would be integration tests rather than unit tests
}
