//! Tests for the pure (non-Tauri-state-dependent) IPC command bodies.
//! Each #[tauri::command] in ipc.rs delegates to an inner function that
//! takes plain args (no Tauri state) so we can test it without spinning up
//! the runtime.

use unreal_merge::ipc::{
    apply_resolution_inner, diff_snapshots_inner, get_app_mode_inner,
};
use unreal_merge::app_mode::AppMode;

#[test]
fn get_app_mode_inner_returns_constructed_value() {
    let mode = AppMode::StandaloneGui;
    assert_eq!(get_app_mode_inner(&mode), mode);
}

#[test]
fn diff_snapshots_inner_returns_empty_for_identical_inputs() {
    use unreal_merge::schema::{Asset, AssetSnapshot, Package};
    let snap = AssetSnapshot {
        id: None,
        ok: true,
        path: None,
        package: Package {
            name: "x".into(),
            engine_version: "5.6".into(),
            file_version_ue5: 1017,
            saved_hash: "sha1:0".into(),
        },
        asset: Asset {
            class: "Blueprint".into(),
            parent_class: "".into(),
            name: "Test".into(),
            properties: vec![],
        },
    };
    let diffs = diff_snapshots_inner(&snap, &snap);
    assert!(diffs.is_empty());
}

#[test]
fn apply_resolution_inner_take_theirs_copies_file() {
    use tempfile::TempDir;
    let tmp = TempDir::new().unwrap();
    let ours = tmp.path().join("ours");
    let theirs = tmp.path().join("theirs");
    let dest = tmp.path().join("dest");
    std::fs::write(&ours, b"OURS").unwrap();
    std::fs::write(&theirs, b"THEIRS").unwrap();
    std::fs::write(&dest, b"STALE").unwrap();
    apply_resolution_inner("theirs", &ours, &theirs, &dest).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"THEIRS");
}
