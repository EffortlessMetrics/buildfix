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

#[cfg(test)]
mod tests {
    use super::*;
    use fs_err as fs;
    use tempfile::TempDir;

    #[test]
    fn fs_repo_view_reads_relative_and_absolute_paths() {
        let temp = TempDir::new().expect("temp dir");
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
        let file_path = root.join("Cargo.toml");
        fs::write(&file_path, "name = \"demo\"").expect("write");

        let repo = FsRepoView::new(root.clone());

        let rel_contents = repo
            .read_to_string(Utf8Path::new("Cargo.toml"))
            .expect("read relative");
        assert_eq!(rel_contents, "name = \"demo\"");

        assert!(file_path.is_absolute());
        let abs_contents = repo.read_to_string(&file_path).expect("read absolute");
        assert_eq!(abs_contents, "name = \"demo\"");
    }

    #[test]
    fn fs_repo_view_exists_checks() {
        let temp = TempDir::new().expect("temp dir");
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
        let file_path = root.join("Cargo.toml");
        fs::write(&file_path, "name = \"demo\"").expect("write");

        let repo = FsRepoView::new(root);
        assert!(repo.exists(Utf8Path::new("Cargo.toml")));
        assert!(!repo.exists(Utf8Path::new("missing.toml")));
    }

    #[test]
    fn fs_repo_view_root_is_stable() {
        let temp = TempDir::new().expect("temp dir");
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
        let repo = FsRepoView::new(root.clone());
        assert_eq!(repo.root(), root.as_path());
    }
}
