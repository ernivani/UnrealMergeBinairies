use std::process::Command;
use tempfile::TempDir;
use unreal_merge::git;

/// Create a tmp git repo with a manually-induced binary conflict between branches.
/// Returns (tmpdir, repo path, conflicted file relative path).
fn create_conflict_repo() -> (TempDir, std::path::PathBuf, String) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(&path)
            .status()
            .unwrap();
        assert!(status.success(), "git {:?} failed", args);
    };

    git(&["init", "-q"]);
    git(&["config", "user.email", "test@example.com"]);
    git(&["config", "user.name", "test"]);
    git(&["checkout", "-b", "main", "-q"]);
    std::fs::write(path.join("Asset.uasset"), b"BASE-CONTENT").unwrap();
    git(&["add", "Asset.uasset"]);
    git(&["commit", "-q", "-m", "base"]);

    git(&["checkout", "-b", "feature", "-q"]);
    std::fs::write(path.join("Asset.uasset"), b"FEATURE-CONTENT").unwrap();
    git(&["commit", "-q", "-am", "feature edit"]);

    git(&["checkout", "main", "-q"]);
    std::fs::write(path.join("Asset.uasset"), b"MAIN-CONTENT").unwrap();
    git(&["commit", "-q", "-am", "main edit"]);

    // Attempt merge - should fail because binary, leaving conflict.
    let _ = Command::new("git")
        .args(["merge", "feature", "--no-edit"])
        .current_dir(&path)
        .status();

    (dir, path, "Asset.uasset".to_string())
}

#[test]
fn lists_uasset_conflicts() {
    let (_tmp, repo, _) = create_conflict_repo();
    let conflicts = git::list_conflicts(&repo).unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0], "Asset.uasset");
}

#[test]
fn reads_stages_to_temp_files() {
    let (_tmp, repo, file) = create_conflict_repo();
    let stages = git::read_stages(&repo, &file).unwrap();
    let base = std::fs::read(&stages.base).unwrap();
    let ours = std::fs::read(&stages.ours).unwrap();
    let theirs = std::fs::read(&stages.theirs).unwrap();
    assert_eq!(base, b"BASE-CONTENT");
    assert_eq!(ours, b"MAIN-CONTENT");
    assert_eq!(theirs, b"FEATURE-CONTENT");
}

#[test]
fn mark_resolved_runs_git_add() {
    let (_tmp, repo, file) = create_conflict_repo();
    // Write a resolution overwriting the working file.
    std::fs::write(repo.join(&file), b"RESOLVED").unwrap();
    git::mark_resolved(&repo, &file).unwrap();
    // After this, the file should no longer be in unmerged state.
    let conflicts = git::list_conflicts(&repo).unwrap();
    assert!(conflicts.is_empty());
}
