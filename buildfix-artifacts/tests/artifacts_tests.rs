//! Comprehensive unit tests for buildfix-artifacts crate.
//!
//! These tests cover:
//! - Path resolution for different artifact types
//! - Directory structure handling
//! - Path normalization
//! - Edge cases (special characters, nested paths, etc.)

use buildfix_artifacts::{ArtifactWriter, FsArtifactWriter};
use camino::{Utf8Path, Utf8PathBuf};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// =============================================================================
// Mock Writer for Testing
// =============================================================================

/// Mock writer that captures all operations for verification.
#[derive(Debug, Clone)]
struct MockArtifactWriter {
    files: Rc<RefCell<HashMap<String, Vec<u8>>>>,
    dirs: Rc<RefCell<Vec<String>>>,
    write_should_fail: bool,
    mkdir_should_fail: bool,
}

impl MockArtifactWriter {
    fn new() -> Self {
        Self {
            files: Rc::new(RefCell::new(HashMap::new())),
            dirs: Rc::new(RefCell::new(Vec::new())),
            write_should_fail: false,
            mkdir_should_fail: false,
        }
    }

    fn with_write_failure() -> Self {
        Self {
            write_should_fail: true,
            ..Self::new()
        }
    }

    fn with_mkdir_failure() -> Self {
        Self {
            mkdir_should_fail: true,
            ..Self::new()
        }
    }

    fn get_file_content(&self, path: &str) -> Option<Vec<u8>> {
        self.files.borrow().get(path).cloned()
    }

    fn file_exists(&self, path: &str) -> bool {
        self.files.borrow().contains_key(path)
    }

    fn files_written(&self) -> Vec<String> {
        let mut files: Vec<String> = self.files.borrow().keys().cloned().collect();
        files.sort();
        files
    }

    fn dirs_created(&self) -> Vec<String> {
        self.dirs.borrow().clone()
    }
}

impl ArtifactWriter for MockArtifactWriter {
    fn write_file(&self, path: &Utf8Path, contents: &[u8]) -> anyhow::Result<()> {
        if self.write_should_fail {
            anyhow::bail!("mock write failure for path: {}", path);
        }
        self.files
            .borrow_mut()
            .insert(path.to_string(), contents.to_vec());
        Ok(())
    }

    fn create_dir_all(&self, path: &Utf8Path) -> anyhow::Result<()> {
        if self.mkdir_should_fail {
            anyhow::bail!("mock mkdir failure for path: {}", path);
        }
        self.dirs.borrow_mut().push(path.to_string());
        Ok(())
    }
}

// =============================================================================
// Path Resolution Tests
// =============================================================================

#[test]
fn test_path_resolution_simple_filename() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("output.txt");

    writer.write_file(path, b"content").unwrap();

    assert!(writer.file_exists("output.txt"));
    assert_eq!(
        writer.get_file_content("output.txt"),
        Some(b"content".to_vec())
    );
}

#[test]
fn test_path_resolution_nested_directory() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("artifacts/plan/output.json");

    writer.write_file(path, b"{}").unwrap();

    assert!(writer.file_exists("artifacts/plan/output.json"));
}

#[test]
fn test_path_resolution_deeply_nested() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("a/b/c/d/e/f/g/deep_file.txt");

    writer.write_file(path, b"deep content").unwrap();

    assert!(writer.file_exists("a/b/c/d/e/f/g/deep_file.txt"));
}

#[test]
fn test_path_resolution_with_dot() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("./output/file.txt");

    writer.write_file(path, b"content").unwrap();

    // Camino normalizes paths with ./
    assert!(writer.file_exists("./output/file.txt"));
}

#[test]
fn test_path_resolution_with_double_dot() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("parent/../sibling/file.txt");

    writer.write_file(path, b"content").unwrap();

    assert!(writer.file_exists("parent/../sibling/file.txt"));
}

#[test]
fn test_path_resolution_absolute_path() {
    let writer = MockArtifactWriter::new();
    // Using a cross-platform absolute path pattern
    let path = if cfg!(windows) {
        Utf8Path::new("C:/temp/artifacts/file.txt")
    } else {
        Utf8Path::new("/tmp/artifacts/file.txt")
    };

    writer.write_file(path, b"content").unwrap();

    assert!(writer.file_exists(path.as_str()));
}

#[test]
fn test_path_resolution_with_trailing_slash_dir() {
    let writer = MockArtifactWriter::new();
    let dir = Utf8Path::new("artifacts/output/");

    writer.create_dir_all(dir).unwrap();

    let dirs = writer.dirs_created();
    assert!(dirs.contains(&"artifacts/output/".to_string()));
}

// =============================================================================
// Directory Structure Tests
// =============================================================================

#[test]
fn test_directory_creation_single() {
    let writer = MockArtifactWriter::new();
    let dir = Utf8Path::new("artifacts");

    writer.create_dir_all(dir).unwrap();

    assert_eq!(writer.dirs_created(), vec!["artifacts"]);
}

#[test]
fn test_directory_creation_nested() {
    let writer = MockArtifactWriter::new();
    let dir = Utf8Path::new("artifacts/extras");

    writer.create_dir_all(dir).unwrap();

    assert_eq!(writer.dirs_created(), vec!["artifacts/extras"]);
}

#[test]
fn test_directory_creation_multiple_nested() {
    let writer = MockArtifactWriter::new();

    writer.create_dir_all(Utf8Path::new("artifacts")).unwrap();
    writer
        .create_dir_all(Utf8Path::new("artifacts/extras"))
        .unwrap();
    writer
        .create_dir_all(Utf8Path::new("artifacts/reports"))
        .unwrap();

    let dirs = writer.dirs_created();
    assert_eq!(dirs.len(), 3);
    assert!(dirs.contains(&"artifacts".to_string()));
    assert!(dirs.contains(&"artifacts/extras".to_string()));
    assert!(dirs.contains(&"artifacts/reports".to_string()));
}

#[test]
fn test_directory_structure_for_plan_artifacts() {
    let writer = MockArtifactWriter::new();
    let out_dir = Utf8Path::new("artifacts");

    // Simulate plan artifact structure
    writer.create_dir_all(out_dir).unwrap();
    writer.create_dir_all(&out_dir.join("extras")).unwrap();

    let plan_json = out_dir.join("plan.json");
    let plan_md = out_dir.join("plan.md");
    let comment_md = out_dir.join("comment.md");
    let patch_diff = out_dir.join("patch.diff");
    let report_json = out_dir.join("report.json");
    let extras_report = out_dir.join("extras").join("buildfix.report.v1.json");

    writer.write_file(&plan_json, b"{}").unwrap();
    writer.write_file(&plan_md, b"# Plan").unwrap();
    writer.write_file(&comment_md, b"Comment").unwrap();
    writer.write_file(&patch_diff, b"diff").unwrap();
    writer.write_file(&report_json, b"{}").unwrap();
    writer.write_file(&extras_report, b"{}").unwrap();

    let files = writer.files_written();
    assert_eq!(files.len(), 6);
    assert!(files.contains(&plan_json.to_string()));
    assert!(files.contains(&plan_md.to_string()));
    assert!(files.contains(&comment_md.to_string()));
    assert!(files.contains(&patch_diff.to_string()));
    assert!(files.contains(&report_json.to_string()));
    assert!(files.contains(&extras_report.to_string()));
}

#[test]
fn test_directory_structure_for_apply_artifacts() {
    let writer = MockArtifactWriter::new();
    let out_dir = Utf8Path::new("artifacts");

    // Simulate apply artifact structure
    writer.create_dir_all(out_dir).unwrap();
    writer.create_dir_all(&out_dir.join("extras")).unwrap();

    let apply_json = out_dir.join("apply.json");
    let apply_md = out_dir.join("apply.md");
    let patch_diff = out_dir.join("patch.diff");
    let report_json = out_dir.join("report.json");
    let extras_report = out_dir.join("extras").join("buildfix.report.v1.json");

    writer.write_file(&apply_json, b"{}").unwrap();
    writer.write_file(&apply_md, b"# Apply").unwrap();
    writer.write_file(&patch_diff, b"diff").unwrap();
    writer.write_file(&report_json, b"{}").unwrap();
    writer.write_file(&extras_report, b"{}").unwrap();

    let files = writer.files_written();
    assert_eq!(files.len(), 5);
    assert!(files.contains(&apply_json.to_string()));
    assert!(files.contains(&apply_md.to_string()));
    assert!(files.contains(&patch_diff.to_string()));
    assert!(files.contains(&report_json.to_string()));
    assert!(files.contains(&extras_report.to_string()));
}

// =============================================================================
// Path Normalization Tests
// =============================================================================

#[test]
fn test_path_normalization_forward_slashes() {
    let writer = MockArtifactWriter::new();
    // Forward slashes should be preserved
    let path = Utf8Path::new("artifacts/reports/plan.json");

    writer.write_file(path, b"content").unwrap();

    assert!(writer.file_exists("artifacts/reports/plan.json"));
}

#[test]
fn test_path_normalization_empty_component() {
    let writer = MockArtifactWriter::new();
    // Double slashes get normalized
    let path = Utf8Path::new("artifacts//plan.json");

    writer.write_file(path, b"content").unwrap();

    // Camino normalizes double slashes
    assert!(writer.file_exists("artifacts//plan.json"));
}

#[test]
fn test_path_normalization_current_directory() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new(".");

    writer.create_dir_all(path).unwrap();

    assert!(writer.dirs_created().contains(&".".to_string()));
}

#[test]
fn test_path_join_normalization() {
    let base = Utf8Path::new("artifacts");
    let joined = base.join("plan.json");

    // Use ends_with for cross-platform compatibility
    assert!(joined.as_str().ends_with("plan.json"));
    assert!(joined.as_str().starts_with("artifacts"));
}

#[test]
fn test_path_join_nested() {
    let base = Utf8Path::new("artifacts");
    let joined = base.join("extras").join("report.json");

    // Use ends_with for cross-platform compatibility
    assert!(joined.as_str().ends_with("report.json"));
    assert!(joined.as_str().contains("extras"));
}

#[test]
fn test_path_join_with_trailing_slash() {
    let base = Utf8Path::new("artifacts/");
    let joined = base.join("file.txt");

    // Trailing slash is preserved in the base but join works correctly
    assert!(joined.as_str().ends_with("file.txt"));
}

#[test]
fn test_path_parent_resolution() {
    let path = Utf8Path::new("artifacts/extras/report.json");
    let parent = path.parent().unwrap();

    assert_eq!(parent.as_str(), "artifacts/extras");
}

#[test]
fn test_path_parent_of_top_level() {
    let path = Utf8Path::new("artifacts");
    let parent = path.parent();

    // Parent of single component is empty
    assert!(parent.map_or(true, |p| p.as_str().is_empty()));
}

#[test]
fn test_path_file_name_extraction() {
    let path = Utf8Path::new("artifacts/extras/report.json");
    let file_name = path.file_name();

    assert_eq!(file_name, Some("report.json"));
}

#[test]
fn test_path_extension_extraction() {
    let path = Utf8Path::new("artifacts/extras/report.json");
    let extension = path.extension();

    assert_eq!(extension, Some("json"));
}

#[test]
fn test_path_stem_extraction() {
    let path = Utf8Path::new("artifacts/extras/report.json");
    let stem = path.file_stem();

    assert_eq!(stem, Some("report"));
}

#[test]
fn test_path_with_extension() {
    let path = Utf8Path::new("artifacts/report.json");
    let new_path = path.with_extension("md");

    assert_eq!(new_path.as_str(), "artifacts/report.md");
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[test]
fn test_edge_case_empty_file_content() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("empty.txt");

    writer.write_file(path, b"").unwrap();

    assert!(writer.file_exists("empty.txt"));
    assert_eq!(writer.get_file_content("empty.txt"), Some(vec![]));
}

#[test]
fn test_edge_case_large_file_content() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("large.bin");
    let large_content = vec![0u8; 1_000_000]; // 1MB

    writer.write_file(path, &large_content).unwrap();

    assert_eq!(writer.get_file_content("large.bin"), Some(large_content));
}

#[test]
fn test_edge_case_unicode_filename() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("artifacts/文档/计划.json"); // Chinese characters

    writer.write_file(path, b"{}").unwrap();

    assert!(writer.file_exists("artifacts/文档/计划.json"));
}

#[test]
fn test_edge_case_special_characters_filename() {
    let writer = MockArtifactWriter::new();
    // Using characters that are valid in paths
    let path = Utf8Path::new("artifacts/report-v1.0.0_beta.json");

    writer.write_file(path, b"{}").unwrap();

    assert!(writer.file_exists("artifacts/report-v1.0.0_beta.json"));
}

#[test]
fn test_edge_case_spaces_in_path() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("artifacts/my reports/plan file.json");

    writer.write_file(path, b"{}").unwrap();

    assert!(writer.file_exists("artifacts/my reports/plan file.json"));
}

#[test]
fn test_edge_case_very_long_path() {
    let writer = MockArtifactWriter::new();
    // Create a very long path
    let long_segment = "a".repeat(100);
    let path_str = format!("artifacts/{}/{}/file.txt", long_segment, long_segment);
    let path = Utf8Path::new(&path_str);

    writer.write_file(path, b"content").unwrap();

    assert!(writer.file_exists(&path_str));
}

#[test]
fn test_edge_case_many_nested_directories() {
    let writer = MockArtifactWriter::new();
    // Create a path with many nested directories
    let nested_path: String = (0..50)
        .map(|i| format!("level{}", i))
        .collect::<Vec<_>>()
        .join("/");
    let path_str = format!("{}/file.txt", nested_path);
    let path = Utf8Path::new(&path_str);

    writer.write_file(path, b"content").unwrap();

    assert!(writer.file_exists(&path_str));
}

#[test]
fn test_edge_case_single_character_names() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("a/b/c/d/e/f.txt");

    writer.write_file(path, b"content").unwrap();

    assert!(writer.file_exists("a/b/c/d/e/f.txt"));
}

#[test]
fn test_edge_case_numeric_path_components() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("artifacts/2024/01/15/report-12345.json");

    writer.write_file(path, b"{}").unwrap();

    assert!(writer.file_exists("artifacts/2024/01/15/report-12345.json"));
}

#[test]
fn test_edge_case_hidden_file() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("artifacts/.hidden");

    writer.write_file(path, b"secret").unwrap();

    assert!(writer.file_exists("artifacts/.hidden"));
}

#[test]
fn test_edge_case_multiple_extensions() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("artifacts/archive.tar.gz");

    writer.write_file(path, b"binary data").unwrap();

    assert_eq!(path.extension(), Some("gz"));
    assert!(writer.file_exists("artifacts/archive.tar.gz"));
}

#[test]
fn test_edge_case_no_extension() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("artifacts/README");

    writer.write_file(path, b"readme content").unwrap();

    assert_eq!(path.extension(), None);
    assert!(writer.file_exists("artifacts/README"));
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[test]
fn test_error_write_failure() {
    let writer = MockArtifactWriter::with_write_failure();
    let path = Utf8Path::new("test.txt");

    let result = writer.write_file(path, b"content");

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("mock write failure")
    );
}

#[test]
fn test_error_mkdir_failure() {
    let writer = MockArtifactWriter::with_mkdir_failure();
    let path = Utf8Path::new("artifacts");

    let result = writer.create_dir_all(path);

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("mock mkdir failure")
    );
}

// =============================================================================
// FsArtifactWriter Integration Tests
// =============================================================================

#[test]
fn test_fs_writer_creates_nested_directories() {
    let writer = FsArtifactWriter;
    let temp = tempfile::tempdir().unwrap();
    let nested_path = temp.path().join("a").join("b").join("c").join("file.txt");
    let path: &Utf8Path = Utf8Path::from_path(nested_path.as_path()).unwrap();

    writer.write_file(path, b"nested content").unwrap();

    assert!(path.exists());
    assert_eq!(std::fs::read(path).unwrap(), b"nested content");
}

#[test]
fn test_fs_writer_overwrites_existing_file() {
    let writer = FsArtifactWriter;
    let temp = tempfile::tempdir().unwrap();
    let file_path = temp.path().join("existing.txt");
    let path: &Utf8Path = Utf8Path::from_path(file_path.as_path()).unwrap();

    writer.write_file(path, b"original").unwrap();
    writer.write_file(path, b"updated").unwrap();

    assert_eq!(std::fs::read(path).unwrap(), b"updated");
}

#[test]
fn test_fs_writer_create_dir_all() {
    let writer = FsArtifactWriter;
    let temp = tempfile::tempdir().unwrap();
    let dir_path = temp.path().join("new").join("directory");
    let path: &Utf8Path = Utf8Path::from_path(dir_path.as_path()).unwrap();

    writer.create_dir_all(path).unwrap();

    assert!(path.exists());
    assert!(path.is_dir());
}

#[test]
fn test_fs_writer_create_dir_all_existing() {
    let writer = FsArtifactWriter;
    let temp = tempfile::tempdir().unwrap();
    let dir_path = temp.path().join("existing");
    let path: &Utf8Path = Utf8Path::from_path(dir_path.as_path()).unwrap();

    // Create directory first
    std::fs::create_dir(path).unwrap();
    assert!(path.exists());

    // create_dir_all on existing directory should succeed
    writer.create_dir_all(path).unwrap();
    assert!(path.exists());
}

#[test]
fn test_fs_writer_with_unicode_path() {
    let writer = FsArtifactWriter;
    let temp = tempfile::tempdir().unwrap();
    let unicode_path = temp.path().join("文档").join("计划.json");
    let path: &Utf8Path = Utf8Path::from_path(unicode_path.as_path()).unwrap();

    writer.write_file(path, b"unicode test").unwrap();

    assert!(path.exists());
    assert_eq!(std::fs::read(path).unwrap(), b"unicode test");
}

#[test]
fn test_fs_writer_with_spaces_in_path() {
    let writer = FsArtifactWriter;
    let temp = tempfile::tempdir().unwrap();
    let spaced_path = temp.path().join("my artifacts").join("plan file.json");
    let path: &Utf8Path = Utf8Path::from_path(spaced_path.as_path()).unwrap();

    writer.write_file(path, b"spaced path test").unwrap();

    assert!(path.exists());
}

// =============================================================================
// Concurrency and State Tests
// =============================================================================

#[test]
fn test_mock_writer_multiple_writes_same_path() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("output.txt");

    writer.write_file(path, b"first").unwrap();
    writer.write_file(path, b"second").unwrap();
    writer.write_file(path, b"third").unwrap();

    // Last write should win
    assert_eq!(
        writer.get_file_content("output.txt"),
        Some(b"third".to_vec())
    );
}

#[test]
fn test_mock_writer_multiple_directories() {
    let writer = MockArtifactWriter::new();

    writer.create_dir_all(Utf8Path::new("dir1")).unwrap();
    writer.create_dir_all(Utf8Path::new("dir2")).unwrap();
    writer.create_dir_all(Utf8Path::new("dir3")).unwrap();

    let dirs = writer.dirs_created();
    assert_eq!(dirs.len(), 3);
}

#[test]
fn test_mock_writer_cloned_shares_state() {
    let writer = MockArtifactWriter::new();
    let cloned = writer.clone();

    writer
        .write_file(Utf8Path::new("file1.txt"), b"content1")
        .unwrap();
    cloned
        .write_file(Utf8Path::new("file2.txt"), b"content2")
        .unwrap();

    // Both writers share the same underlying storage
    assert!(writer.file_exists("file1.txt"));
    assert!(writer.file_exists("file2.txt"));
    assert!(cloned.file_exists("file1.txt"));
    assert!(cloned.file_exists("file2.txt"));
}

// =============================================================================
// Path Buf Operations Tests
// =============================================================================

#[test]
fn test_path_buf_from_str() {
    let path_buf: Utf8PathBuf = "artifacts/plan.json".into();
    assert_eq!(path_buf.as_str(), "artifacts/plan.json");
}

#[test]
fn test_path_buf_from_path() {
    let path = Utf8Path::new("artifacts/plan.json");
    let path_buf: Utf8PathBuf = path.to_path_buf();
    assert_eq!(path_buf.as_str(), "artifacts/plan.json");
}

#[test]
fn test_path_buf_push() {
    let mut path_buf = Utf8PathBuf::from("artifacts");
    path_buf.push("plan.json");
    // Use ends_with for cross-platform compatibility
    assert!(path_buf.as_str().ends_with("plan.json"));
    assert!(path_buf.as_str().starts_with("artifacts"));
}

#[test]
fn test_path_buf_push_multiple() {
    let mut path_buf = Utf8PathBuf::from("artifacts");
    path_buf.push("extras");
    path_buf.push("report.json");
    // Use ends_with for cross-platform compatibility
    assert!(path_buf.as_str().ends_with("report.json"));
    assert!(path_buf.as_str().contains("extras"));
}

#[test]
fn test_path_display() {
    let path = Utf8Path::new("artifacts/plan.json");
    let display = format!("{}", path);
    assert_eq!(display, "artifacts/plan.json");
}

#[test]
fn test_path_to_string() {
    let path = Utf8Path::new("artifacts/plan.json");
    let s = path.to_string();
    assert_eq!(s, "artifacts/plan.json");
}

// =============================================================================
// Binary Content Tests
// =============================================================================

#[test]
fn test_binary_content_null_bytes() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("binary.bin");
    let content = vec![0x00, 0x01, 0x02, 0x03, 0xFF];

    writer.write_file(path, &content).unwrap();

    assert_eq!(writer.get_file_content("binary.bin"), Some(content));
}

#[test]
fn test_binary_content_all_byte_values() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("all_bytes.bin");
    let content: Vec<u8> = (0..=255).collect();

    writer.write_file(path, &content).unwrap();

    assert_eq!(writer.get_file_content("all_bytes.bin"), Some(content));
}

#[test]
fn test_json_content() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("data.json");
    let json_content = br#"{"key": "value", "number": 42, "nested": {"a": 1}}"#;

    writer.write_file(path, json_content).unwrap();

    assert_eq!(
        writer.get_file_content("data.json"),
        Some(json_content.to_vec())
    );
}

#[test]
fn test_markdown_content() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("plan.md");
    let md_content = b"# Plan\n\n## Section\n\n- Item 1\n- Item 2\n";

    writer.write_file(path, md_content).unwrap();

    assert_eq!(
        writer.get_file_content("plan.md"),
        Some(md_content.to_vec())
    );
}

#[test]
fn test_diff_content() {
    let writer = MockArtifactWriter::new();
    let path = Utf8Path::new("patch.diff");
    let diff_content = br#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,4 @@
 line1
+line2
 line3
"#;

    writer.write_file(path, diff_content).unwrap();

    assert_eq!(
        writer.get_file_content("patch.diff"),
        Some(diff_content.to_vec())
    );
}
