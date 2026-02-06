//! Port traits abstracting all I/O away from the pipeline.

use buildfix_receipts::LoadedReceipt;
use camino::Utf8Path;

/// Source of sensor receipts.
pub trait ReceiptSource {
    fn load_receipts(&self) -> anyhow::Result<Vec<LoadedReceipt>>;
}

/// Git queries (HEAD SHA, dirty status).
pub trait GitPort {
    fn head_sha(&self, repo_root: &Utf8Path) -> anyhow::Result<Option<String>>;
    fn is_dirty(&self, repo_root: &Utf8Path) -> anyhow::Result<Option<bool>>;
}

/// File-system write operations.
pub trait WritePort {
    fn write_file(&self, path: &Utf8Path, contents: &[u8]) -> anyhow::Result<()>;
    fn create_dir_all(&self, path: &Utf8Path) -> anyhow::Result<()>;
}
