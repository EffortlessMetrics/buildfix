//! Default filesystem-backed port implementations.

use crate::ports::{GitPort, ReceiptSource, WritePort};
use anyhow::Context;
use buildfix_receipts::LoadedReceipt;
use camino::{Utf8Path, Utf8PathBuf};

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
