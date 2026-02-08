//! Default filesystem-backed port implementations.

use crate::ports::{GitPort, ReceiptSource, WritePort};
use anyhow::Context;
use buildfix_receipts::LoadedReceipt;
use camino::{Utf8Path, Utf8PathBuf};
use tracing::debug;

/// Loads receipts from the filesystem via `buildfix_receipts::load_receipts`.
#[derive(Debug, Clone)]
pub struct FsReceiptSource {
    pub artifacts_dir: Utf8PathBuf,
}

impl FsReceiptSource {
    pub fn new(artifacts_dir: Utf8PathBuf) -> Self {
        Self { artifacts_dir }
    }
}

impl ReceiptSource for FsReceiptSource {
    fn load_receipts(&self) -> anyhow::Result<Vec<LoadedReceipt>> {
        buildfix_receipts::load_receipts(&self.artifacts_dir)
            .with_context(|| format!("load receipts from {}", self.artifacts_dir))
    }
}

/// Git operations via `buildfix_edit` shell helpers.
#[derive(Debug, Clone, Default)]
pub struct ShellGitPort;

impl GitPort for ShellGitPort {
    fn head_sha(&self, repo_root: &Utf8Path) -> anyhow::Result<Option<String>> {
        match buildfix_edit::get_head_sha(repo_root) {
            Ok(sha) => Ok(Some(sha)),
            Err(_) => Ok(None),
        }
    }

    fn is_dirty(&self, repo_root: &Utf8Path) -> anyhow::Result<Option<bool>> {
        match buildfix_edit::is_working_tree_dirty(repo_root) {
            Ok(dirty) => Ok(Some(dirty)),
            Err(_) => Ok(None),
        }
    }
}

/// In-memory receipt source for embedding and testing.
///
/// Accepts pre-loaded receipts, filters out reserved non-sensor receipts
/// (buildfix, cockpit) by `sensor_id` **or** path prefix (belt-and-suspenders),
/// mirroring the fs loader's self-ingest guard, and sorts by path on
/// construction to match `FsReceiptSource`'s deterministic ordering.
#[derive(Debug, Clone)]
pub struct InMemoryReceiptSource {
    receipts: Vec<LoadedReceipt>,
}

impl InMemoryReceiptSource {
    pub fn new(mut receipts: Vec<LoadedReceipt>) -> Self {
        receipts.retain(|r| {
            let sid = r.sensor_id.as_str().to_ascii_lowercase();
            let p = r.path.as_str().replace('\\', "/");
            let p = p.to_ascii_lowercase();

            let is_buildfix = sid == "buildfix"
                || p.starts_with("artifacts/buildfix/")
                || p.contains("/artifacts/buildfix/");
            let is_cockpit = sid == "cockpit"
                || p.starts_with("artifacts/cockpit/")
                || p.contains("/artifacts/cockpit/");

            if is_buildfix || is_cockpit {
                debug!(
                    path = r.path.as_str(),
                    sensor_id = r.sensor_id.as_str(),
                    "skipping non-sensor receipt"
                );
                return false;
            }
            true
        });
        receipts.sort_by(|a, b| a.path.cmp(&b.path));
        Self { receipts }
    }
}

impl ReceiptSource for InMemoryReceiptSource {
    fn load_receipts(&self) -> anyhow::Result<Vec<LoadedReceipt>> {
        Ok(self.receipts.clone())
    }
}

/// Filesystem write operations.
#[derive(Debug, Clone, Default)]
pub struct FsWritePort;

impl WritePort for FsWritePort {
    fn write_file(&self, path: &Utf8Path, contents: &[u8]) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create parent dir for {}", path))?;
        }
        std::fs::write(path, contents).with_context(|| format!("write {}", path))
    }

    fn create_dir_all(&self, path: &Utf8Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(path).with_context(|| format!("create_dir_all {}", path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buildfix_receipts::ReceiptLoadError;

    fn make_receipt(path: &str) -> LoadedReceipt {
        LoadedReceipt {
            path: Utf8PathBuf::from(path),
            sensor_id: "test".to_string(),
            receipt: Err(ReceiptLoadError::Io {
                message: "stub".to_string(),
            }),
        }
    }

    fn make_receipt_with_sensor(path: &str, sensor_id: &str) -> LoadedReceipt {
        LoadedReceipt {
            path: Utf8PathBuf::from(path),
            sensor_id: sensor_id.to_string(),
            receipt: Err(ReceiptLoadError::Io {
                message: "stub".to_string(),
            }),
        }
    }

    #[test]
    fn in_memory_sorts_by_path() {
        let source = InMemoryReceiptSource::new(vec![
            make_receipt("artifacts/z-sensor/report.json"),
            make_receipt("artifacts/a-sensor/report.json"),
            make_receipt("artifacts/m-sensor/report.json"),
        ]);
        let loaded = source.load_receipts().unwrap();
        let paths: Vec<&str> = loaded.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "artifacts/a-sensor/report.json",
                "artifacts/m-sensor/report.json",
                "artifacts/z-sensor/report.json",
            ]
        );
    }

    #[test]
    fn in_memory_preserves_errors() {
        let source = InMemoryReceiptSource::new(vec![make_receipt("artifacts/bad/report.json")]);
        let loaded = source.load_receipts().unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(loaded[0].receipt.is_err());
    }

    #[test]
    fn in_memory_empty_source() {
        let source = InMemoryReceiptSource::new(vec![]);
        let loaded = source.load_receipts().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn in_memory_filters_buildfix_by_sensor_id() {
        let source = InMemoryReceiptSource::new(vec![make_receipt_with_sensor(
            "some/arbitrary/path.json",
            "buildfix",
        )]);
        let loaded = source.load_receipts().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn in_memory_filters_buildfix_by_path() {
        let source = InMemoryReceiptSource::new(vec![make_receipt_with_sensor(
            "artifacts/buildfix/report.json",
            "unknown",
        )]);
        let loaded = source.load_receipts().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn in_memory_filters_cockpit_by_sensor_id() {
        let source = InMemoryReceiptSource::new(vec![make_receipt_with_sensor(
            "some/arbitrary/path.json",
            "cockpit",
        )]);
        let loaded = source.load_receipts().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn in_memory_filters_cockpit_by_path() {
        let source = InMemoryReceiptSource::new(vec![make_receipt_with_sensor(
            "artifacts/cockpit/report.json",
            "unknown",
        )]);
        let loaded = source.load_receipts().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn in_memory_filters_buildfix_by_backslash_path() {
        let source = InMemoryReceiptSource::new(vec![make_receipt_with_sensor(
            r"artifacts\buildfix\report.json",
            "unknown",
        )]);
        let loaded = source.load_receipts().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn in_memory_filters_cockpit_by_absolute_path() {
        let source = InMemoryReceiptSource::new(vec![make_receipt_with_sensor(
            r"C:\repo\artifacts\cockpit\report.json",
            "unknown",
        )]);
        let loaded = source.load_receipts().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn in_memory_filters_case_insensitive_path() {
        let source = InMemoryReceiptSource::new(vec![make_receipt_with_sensor(
            r"C:\repo\Artifacts\cockpit\report.json",
            "unknown",
        )]);
        let loaded = source.load_receipts().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn in_memory_filters_reserved_among_others() {
        let source = InMemoryReceiptSource::new(vec![
            make_receipt_with_sensor("artifacts/z-sensor/report.json", "z-sensor"),
            make_receipt_with_sensor("artifacts/buildfix/report.json", "buildfix"),
            make_receipt_with_sensor("artifacts/cockpit/report.json", "unknown"),
            make_receipt_with_sensor("artifacts/a-sensor/report.json", "a-sensor"),
        ]);
        let loaded = source.load_receipts().unwrap();
        let paths: Vec<&str> = loaded.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "artifacts/a-sensor/report.json",
                "artifacts/z-sensor/report.json",
            ]
        );
    }
}
