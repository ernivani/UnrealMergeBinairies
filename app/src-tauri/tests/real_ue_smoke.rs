use std::path::PathBuf;
use unreal_merge::sidecar::{Sidecar, SidecarConfig};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Run with: cargo test --test real_ue_smoke -- --ignored --nocapture
#[test]
#[ignore]
fn export_v1_fixture_via_real_ue() {
    let root = repo_root();
    let ue = PathBuf::from(
        r"C:\Program Files\Epic Games\UE_5.6\Engine\Binaries\Win64\UnrealEditor.exe",
    );
    assert!(ue.exists(), "UE 5.6 not installed at {}", ue.display());

    let host_project = root.join("ue-host").join("HostProject.uproject");
    assert!(host_project.exists());

    let v1 = root.join("Examples").join("v1").join("BP_MinimalChar.uasset");
    assert!(v1.exists());

    let log_path = std::env::temp_dir().join("unreal-merge-smoke.log");
    let cfg = SidecarConfig {
        executable: ue,
        args: vec![
            host_project.to_string_lossy().to_string(),
            "-run=MergeBinariesExport".to_string(),
            "-stdio".to_string(),
            "-nullrhi".to_string(),
            "-unattended".to_string(),
            "-NoCrashReports".to_string(),
        ],
        prepend_warmup: true,
        log_redirect: Some(log_path),
    };
    let sidecar = Sidecar::new(cfg);

    let v1_str = v1.to_string_lossy().replace('\\', "/");
    let requests = vec![serde_json::json!({"id": 1, "cmd": "export", "path": v1_str})];
    let responses = sidecar.run_batch(&requests).expect("run batch");

    let response = responses
        .iter()
        .find(|r| r.get("id").and_then(|i| i.as_u64()) == Some(1))
        .expect("id=1 response from real UE 5.6 sidecar");

    assert_eq!(response["ok"], true, "got: {}", serde_json::to_string_pretty(response).unwrap());
    assert_eq!(response["asset"]["class"], "Blueprint");
    assert_eq!(response["package"]["fileVersionUE5"], 1017);
    let props = response["asset"]["properties"].as_array().expect("properties array");
    assert!(props.len() >= 30, "expected >= 30 properties, got {}", props.len());
}
