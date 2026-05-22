use pretty_assertions::assert_eq;
use std::path::PathBuf;
use unreal_merge::schema::AssetSnapshot;

fn read_json(path: &std::path::Path) -> String {
    let raw = std::fs::read_to_string(path).expect("read golden");
    // Goldens were written via PowerShell which prepends a UTF-8 BOM.
    raw.strip_prefix('\u{FEFF}').map(|s| s.to_string()).unwrap_or(raw)
}

fn repo_root() -> PathBuf {
    // workspace root is two levels up from CARGO_MANIFEST_DIR (app/src-tauri/)
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn parse_v1_golden() {
    let path = repo_root().join("Examples").join("v1.expected.json");
    let raw = read_json(&path);
    let snap: AssetSnapshot = serde_json::from_str(&raw).expect("parse v1 golden");

    assert_eq!(snap.ok, true);
    assert_eq!(snap.package.name, "/MergeTmp/BP_MinimalChar");
    assert_eq!(snap.package.file_version_ue5, 1017);
    assert!(snap.package.saved_hash.starts_with("sha1:"));
    assert_eq!(snap.asset.class, "Blueprint");
    assert!(snap.asset.properties.len() > 30);
}

#[test]
fn v1_and_v2_have_same_property_count() {
    let v1: AssetSnapshot =
        serde_json::from_str(&read_json(&repo_root().join("Examples/v1.expected.json"))).unwrap();
    let v2: AssetSnapshot =
        serde_json::from_str(&read_json(&repo_root().join("Examples/v2.expected.json"))).unwrap();
    // Plan 1's done report: at this schema depth v1 and v2 are byte-identical
    // except savedHash. Lock this expectation so a future regression that
    // accidentally walks deeper into structs is flagged immediately.
    assert_eq!(v1.asset.properties.len(), v2.asset.properties.len());
    assert_ne!(v1.package.saved_hash, v2.package.saved_hash);
}
