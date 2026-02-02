use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;

/// Read-only repository access.
///
/// buildfix-domain uses this so it can be tested against an in-memory implementation later.
pub trait RepoView {
    fn root(&self) -> &Utf8Path;

    fn read_to_string(&self, rel: &Utf8Path) -> anyhow::Result<String>;

    fn exists(&self, rel: &Utf8Path) -> bool;
}

/// File-system backed `RepoView`.
#[derive(Debug, Clone)]
pub struct FsRepoView {
    root: Utf8PathBuf,
}

impl FsRepoView {
    pub fn new(root: Utf8PathBuf) -> Self {
        Self { root }
    }

    fn abs(&self, rel: &Utf8Path) -> Utf8PathBuf {
        if rel.is_absolute() {
            rel.to_path_buf()
        } else {
            self.root.join(rel)
        }
    }
}

impl RepoView for FsRepoView {
    fn root(&self) -> &Utf8Path {
        &self.root
    }

    fn read_to_string(&self, rel: &Utf8Path) -> anyhow::Result<String> {
        let abs = self.abs(rel);
        fs::read_to_string(&abs).with_context(|| format!("read {}", abs))
    }

    fn exists(&self, rel: &Utf8Path) -> bool {
        self.abs(rel).exists()
    }
}
