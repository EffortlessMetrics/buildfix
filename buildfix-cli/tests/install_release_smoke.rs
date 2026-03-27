use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn buildfix() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("buildfix")
}

fn copy_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("create destination directory");

    for entry in fs::read_dir(src).expect("read source directory") {
        let entry = entry.expect("directory entry");
        let file_type = entry.file_type().expect("file type");
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_tree(&src_path, &dst_path);
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path).expect("copy file");
        } else if file_type.is_symlink() {
            let target = fs::read_link(&src_path).expect("read symlink");
            #[cfg(windows)]
            {
                let metadata = fs::metadata(&src_path).expect("symlink metadata");
                if metadata.is_dir() {
                    std::os::windows::fs::symlink_dir(&target, &dst_path)
                        .expect("copy symlink dir");
                } else {
                    std::os::windows::fs::symlink_file(&target, &dst_path)
                        .expect("copy symlink file");
                }
            }
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(&target, &dst_path).expect("copy symlink");
            }
        }
    }
}

fn stage_demo_workspace() -> TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo_dst = temp.path();
    let demo_repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/demo/repo");
    let demo_artifacts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/demo/artifacts");

    copy_tree(&demo_repo, repo_dst);
    copy_tree(&demo_artifacts, &repo_dst.join("artifacts"));

    temp
}

fn read_file(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path).expect("read file")
}

fn manifest_paths(root: &Path) -> Vec<PathBuf> {
    vec![
        root.join("Cargo.toml"),
        root.join("crates").join("api").join("Cargo.toml"),
        root.join("crates").join("cli").join("Cargo.toml"),
        root.join("crates").join("core").join("Cargo.toml"),
    ]
}

#[test]
fn install_release_smoke_supported_lane_works_end_to_end() {
    let temp = stage_demo_workspace();
    let root = temp.path();

    let before_manifests: Vec<(PathBuf, String)> = manifest_paths(root)
        .into_iter()
        .map(|path| {
            let contents = read_file(&path);
            (path, contents)
        })
        .collect();

    buildfix().current_dir(root).arg("plan").assert().success();

    for file in ["plan.json", "plan.md", "patch.diff", "report.json"] {
        assert!(root.join("artifacts").join("buildfix").join(file).exists());
    }

    let plan_manifests: Vec<(PathBuf, String)> = manifest_paths(root)
        .into_iter()
        .map(|path| {
            let contents = read_file(&path);
            (path, contents)
        })
        .collect();
    assert_eq!(before_manifests, plan_manifests);

    buildfix().current_dir(root).arg("apply").assert().success();

    let dry_run_manifests: Vec<(PathBuf, String)> = manifest_paths(root)
        .into_iter()
        .map(|path| {
            let contents = read_file(&path);
            (path, contents)
        })
        .collect();
    assert_eq!(plan_manifests, dry_run_manifests);

    for file in ["apply.json", "apply.md", "patch.diff", "report.json"] {
        assert!(root.join("artifacts").join("buildfix").join(file).exists());
    }

    buildfix()
        .current_dir(root)
        .args(["apply", "--apply"])
        .assert()
        .success();

    let after_apply_manifests: Vec<(PathBuf, String)> = manifest_paths(root)
        .into_iter()
        .map(|path| {
            let contents = read_file(&path);
            (path, contents)
        })
        .collect();
    assert_ne!(before_manifests, after_apply_manifests);
}

#[test]
fn install_release_smoke_policy_block_returns_exit_2() {
    let temp = stage_demo_workspace();
    let root = temp.path();
    fs::write(root.join("buildfix.toml"), "[policy]\nmax_ops = 1\n").expect("write policy");

    buildfix().current_dir(root).arg("plan").assert().code(2);
}

#[test]
fn install_release_smoke_stale_plan_returns_exit_2() {
    let temp = stage_demo_workspace();
    let root = temp.path();

    buildfix().current_dir(root).arg("plan").assert().success();

    let cargo_toml = root.join("Cargo.toml");
    let original = read_file(&cargo_toml);
    fs::write(&cargo_toml, format!("{original}\n# stale-plan-smoke\n")).expect("stale edit");

    buildfix()
        .current_dir(root)
        .args(["apply", "--apply"])
        .assert()
        .code(2);
}
