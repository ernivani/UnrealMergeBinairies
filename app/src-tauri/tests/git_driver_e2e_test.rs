use assert_cmd::Command as AssertCommand;
use std::process::Command;
use tempfile::TempDir;

fn git(args: &[&str], cwd: &std::path::Path) {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .unwrap();
    assert!(status.success(), "git {:?} in {} failed", args, cwd.display());
}

#[test]
fn git_driver_theirs_resolves_uasset_conflict() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path();
    git(&["init", "-q"], repo);
    git(&["config", "user.email", "t@e.com"], repo);
    git(&["config", "user.name", "t"], repo);
    git(&["checkout", "-b", "main", "-q"], repo);
    std::fs::write(repo.join("a.uasset"), b"BASE").unwrap();
    git(&["add", "a.uasset"], repo);
    git(&["commit", "-q", "-m", "base"], repo);

    git(&["checkout", "-b", "feature", "-q"], repo);
    std::fs::write(repo.join("a.uasset"), b"FEATURE").unwrap();
    git(&["commit", "-q", "-am", "feature"], repo);

    git(&["checkout", "main", "-q"], repo);
    std::fs::write(repo.join("a.uasset"), b"MAIN").unwrap();
    git(&["commit", "-q", "-am", "main"], repo);

    // Attempt merge - expected to leave a conflict (binary file, no built-in merge).
    let _ = Command::new("git")
        .args(["merge", "feature", "--no-edit"])
        .current_dir(repo)
        .status();

    // Confirm the conflict actually landed.
    let ls = Command::new("git")
        .args(["ls-files", "-u"])
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&ls.stdout).contains("a.uasset"),
        "expected a.uasset in unmerged listing"
    );

    // Stage 2 = ours, stage 3 = theirs. Materialise them via git show.
    let ours_path = repo.join("ours.tmp");
    let theirs_path = repo.join("theirs.tmp");
    std::fs::write(
        &ours_path,
        Command::new("git")
            .args(["show", ":2:a.uasset"])
            .current_dir(repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    std::fs::write(
        &theirs_path,
        Command::new("git")
            .args(["show", ":3:a.uasset"])
            .current_dir(repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();

    // Invoke unreal-merge --git-driver with UNREAL_MERGE_RESOLUTION=theirs.
    AssertCommand::cargo_bin("unreal-merge")
        .unwrap()
        .env("UNREAL_MERGE_RESOLUTION", "theirs")
        .current_dir(repo)
        .args([
            "--git-driver",
            "<base-not-used-for-stage-copy>",
            ours_path.to_string_lossy().as_ref(),
            theirs_path.to_string_lossy().as_ref(),
            "a.uasset",
        ])
        .assert()
        .success();

    // The working tree's `ours.tmp` should now hold the contents of theirs ("FEATURE").
    // (Note: --git-driver's `%A` is the OURS path which Git also uses as the
    // destination after the driver returns. We mimic that here by writing the
    // resolved output back over ours_path.)
    let content = std::fs::read(&ours_path).unwrap();
    assert_eq!(content, b"FEATURE", "Resolution=Theirs should have written FEATURE");
}

#[test]
fn git_driver_abort_exits_nonzero() {
    let tmp = TempDir::new().unwrap();
    let ours = tmp.path().join("o.tmp");
    let theirs = tmp.path().join("t.tmp");
    std::fs::write(&ours, b"OURS").unwrap();
    std::fs::write(&theirs, b"THEIRS").unwrap();

    AssertCommand::cargo_bin("unreal-merge")
        .unwrap()
        .env("UNREAL_MERGE_RESOLUTION", "abort")
        .args([
            "--git-driver",
            "base-irrelevant",
            ours.to_string_lossy().as_ref(),
            theirs.to_string_lossy().as_ref(),
            "a.uasset",
        ])
        .assert()
        .failure(); // Git's signal that the conflict was NOT resolved.

    // Ours must NOT have been overwritten.
    assert_eq!(std::fs::read(&ours).unwrap(), b"OURS");
}
