use pretty_assertions::assert_eq;
use std::path::PathBuf;
use unreal_merge::diff::{PropertyChange, diff_properties};
use unreal_merge::schema::AssetSnapshot;

fn load_snapshot(name: &str) -> AssetSnapshot {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let path = root.join("Examples").join(name);
    let raw = std::fs::read_to_string(&path).unwrap();
    // Goldens are written with UTF-8 BOM by PowerShell; strip it.
    let trimmed = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
    serde_json::from_str(trimmed).unwrap()
}

#[test]
fn v1_vs_v2_no_property_diffs_at_current_depth() {
    let v1 = load_snapshot("v1.expected.json");
    let v2 = load_snapshot("v2.expected.json");
    let diffs = diff_properties(&v1.asset.properties, &v2.asset.properties);
    // Plan 1's done report: shallow walk produces identical property arrays.
    assert!(diffs.is_empty(), "Expected no property diffs, got: {:#?}", diffs);
}

#[test]
fn detects_added_property() {
    use unreal_merge::schema::{Property, PropertyValue};
    let base: Vec<Property> = vec![Property {
        path: "FieldA".to_string(),
        ty: "bool".to_string(),
        value: PropertyValue::Bool(true),
    }];
    let mut other = base.clone();
    other.push(Property {
        path: "FieldB".to_string(),
        ty: "int32".to_string(),
        value: PropertyValue::Number(7.into()),
    });
    let diffs = diff_properties(&base, &other);
    assert_eq!(diffs.len(), 1);
    match &diffs[0] {
        PropertyChange::Added { path, .. } => assert_eq!(path, "FieldB"),
        other => panic!("expected Added, got {:?}", other),
    }
}

#[test]
fn detects_removed_property() {
    use unreal_merge::schema::{Property, PropertyValue};
    let base = vec![
        Property {
            path: "FieldA".to_string(),
            ty: "bool".to_string(),
            value: PropertyValue::Bool(true),
        },
        Property {
            path: "FieldB".to_string(),
            ty: "int32".to_string(),
            value: PropertyValue::Number(7.into()),
        },
    ];
    let other = base[..1].to_vec();
    let diffs = diff_properties(&base, &other);
    assert_eq!(diffs.len(), 1);
    match &diffs[0] {
        PropertyChange::Removed { path, .. } => assert_eq!(path, "FieldB"),
        other => panic!("expected Removed, got {:?}", other),
    }
}

#[test]
fn detects_changed_property() {
    use unreal_merge::schema::{Property, PropertyValue};
    let base = vec![Property {
        path: "Speed".to_string(),
        ty: "float".to_string(),
        value: PropertyValue::Number(serde_json::Number::from_f64(600.0).unwrap()),
    }];
    let mut other = base.clone();
    other[0].value = PropertyValue::Number(serde_json::Number::from_f64(750.0).unwrap());
    let diffs = diff_properties(&base, &other);
    assert_eq!(diffs.len(), 1);
    match &diffs[0] {
        PropertyChange::Changed { path, .. } => assert_eq!(path, "Speed"),
        other => panic!("expected Changed, got {:?}", other),
    }
}
