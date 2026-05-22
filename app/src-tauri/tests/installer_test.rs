use std::process::Command;
use tempfile::TempDir;
use unreal_merge::installer;

fn init_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(&path)
        .status()
        .unwrap();
    (dir, path)
}

#[test]
fn install_writes_gitattributes_and_config() {
    let (_tmp, repo) = init_repo();
    installer::install(&repo, &std::path::PathBuf::from("/usr/bin/unreal-merge")).unwrap();
    let attrs = std::fs::read_to_string(repo.join(".gitattributes")).unwrap();
    assert!(attrs.contains("*.uasset merge=unrealbin"));
    assert!(attrs.contains("*.umap   merge=unrealbin"));

    let cfg = std::fs::read_to_string(repo.join(".git").join("config")).unwrap();
    assert!(cfg.contains("[merge \"unrealbin\"]"));
    assert!(cfg.contains("--git-driver %O %A %B %P"));
}

#[test]
fn install_is_idempotent() {
    let (_tmp, repo) = init_repo();
    installer::install(&repo, &std::path::PathBuf::from("/usr/bin/unreal-merge")).unwrap();
    installer::install(&repo, &std::path::PathBuf::from("/usr/bin/unreal-merge")).unwrap();
    let attrs = std::fs::read_to_string(repo.join(".gitattributes")).unwrap();
    // Only one occurrence of our merge=unrealbin attribute (not two).
    assert_eq!(attrs.matches("*.uasset merge=unrealbin").count(), 1);
}

#[test]
fn uninstall_removes_attrs_and_config() {
    let (_tmp, repo) = init_repo();
    installer::install(&repo, &std::path::PathBuf::from("/usr/bin/unreal-merge")).unwrap();
    installer::uninstall(&repo).unwrap();
    let attrs = std::fs::read_to_string(repo.join(".gitattributes")).unwrap_or_default();
    assert!(!attrs.contains("merge=unrealbin"));
    let cfg = std::fs::read_to_string(repo.join(".git").join("config")).unwrap_or_default();
    assert!(!cfg.contains("[merge \"unrealbin\"]"));
}

#[test]
fn install_preserves_existing_gitattributes() {
    let (_tmp, repo) = init_repo();
    std::fs::write(repo.join(".gitattributes"), "*.txt text\n").unwrap();
    installer::install(&repo, &std::path::PathBuf::from("/usr/bin/unreal-merge")).unwrap();
    let attrs = std::fs::read_to_string(repo.join(".gitattributes")).unwrap();
    assert!(attrs.contains("*.txt text"));
    assert!(attrs.contains("*.uasset merge=unrealbin"));
}
