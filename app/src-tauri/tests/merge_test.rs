use tempfile::TempDir;
use unreal_merge::merge::{Resolution, apply_resolution};

#[test]
fn ours_copies_ours_over_dest() {
    let tmp = TempDir::new().unwrap();
    let ours = tmp.path().join("ours.bin");
    let theirs = tmp.path().join("theirs.bin");
    let dest = tmp.path().join("dest.bin");
    std::fs::write(&ours, b"OURS").unwrap();
    std::fs::write(&theirs, b"THEIRS").unwrap();
    std::fs::write(&dest, b"STALE").unwrap();
    apply_resolution(Resolution::Ours, &ours, &theirs, &dest).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"OURS");
}

#[test]
fn theirs_copies_theirs_over_dest() {
    let tmp = TempDir::new().unwrap();
    let ours = tmp.path().join("ours.bin");
    let theirs = tmp.path().join("theirs.bin");
    let dest = tmp.path().join("dest.bin");
    std::fs::write(&ours, b"OURS").unwrap();
    std::fs::write(&theirs, b"THEIRS").unwrap();
    std::fs::write(&dest, b"STALE").unwrap();
    apply_resolution(Resolution::Theirs, &ours, &theirs, &dest).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"THEIRS");
}

#[test]
fn abort_returns_error() {
    let tmp = TempDir::new().unwrap();
    let ours = tmp.path().join("ours.bin");
    let theirs = tmp.path().join("theirs.bin");
    let dest = tmp.path().join("dest.bin");
    std::fs::write(&ours, b"OURS").unwrap();
    std::fs::write(&theirs, b"THEIRS").unwrap();
    std::fs::write(&dest, b"STALE").unwrap();
    let err = apply_resolution(Resolution::Abort, &ours, &theirs, &dest);
    assert!(err.is_err(), "Abort should produce an error");
    assert_eq!(std::fs::read(&dest).unwrap(), b"STALE", "dest must be untouched");
}

#[test]
fn handles_readonly_dest() {
    let tmp = TempDir::new().unwrap();
    let ours = tmp.path().join("ours.bin");
    let theirs = tmp.path().join("theirs.bin");
    let dest = tmp.path().join("dest.bin");
    std::fs::write(&ours, b"OURS").unwrap();
    std::fs::write(&theirs, b"THEIRS").unwrap();
    std::fs::write(&dest, b"STALE").unwrap();
    // Mark dest read-only (simulates LFS lockable).
    let mut perms = std::fs::metadata(&dest).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&dest, perms).unwrap();
    apply_resolution(Resolution::Theirs, &ours, &theirs, &dest).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"THEIRS");
}
