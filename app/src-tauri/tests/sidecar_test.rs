use std::collections::HashMap;
use unreal_merge::sidecar::{Sidecar, SidecarConfig};

fn mock_path() -> std::path::PathBuf {
    // assert_cmd's helper gives us the path to a binary built by Cargo.
    assert_cmd::cargo::cargo_bin("mock_ue_sidecar")
}

#[test]
fn round_trips_ping_via_mock() {
    let cfg = SidecarConfig {
        executable: mock_path(),
        args: vec![],
        prepend_warmup: true,
        log_redirect: None,
    };
    let sidecar = Sidecar::new(cfg);
    let requests = vec![
        serde_json::json!({"id": 1, "cmd": "ping"}),
        serde_json::json!({"id": 2, "cmd": "quit"}),
    ];
    let responses = sidecar.run_batch(&requests).expect("run batch");
    // Filter to responses with id >= 1 (drop warmup id=0).
    let by_id: HashMap<u64, serde_json::Value> = responses
        .iter()
        .filter_map(|v| v.get("id").and_then(|i| i.as_u64()).map(|id| (id, v.clone())))
        .collect();
    assert_eq!(by_id.len(), 2, "got: {:#?}", responses);
    assert_eq!(by_id[&1]["pong"], "mock_ue_sidecar");
    assert_eq!(by_id[&2]["ok"], true);
}

#[test]
fn export_against_mock_returns_blueprint_snapshot() {
    let cfg = SidecarConfig {
        executable: mock_path(),
        args: vec![],
        prepend_warmup: true,
        log_redirect: None,
    };
    let sidecar = Sidecar::new(cfg);
    let resps = sidecar
        .run_batch(&[
            serde_json::json!({"id": 1, "cmd": "export", "path": "X:/foo.uasset"}),
            serde_json::json!({"id": 2, "cmd": "quit"}),
        ])
        .expect("batch");
    let export = resps
        .iter()
        .find(|r| r.get("id").and_then(|i| i.as_u64()) == Some(1))
        .expect("export response");
    assert_eq!(export["ok"], true);
    assert_eq!(export["asset"]["class"], "Blueprint");
}
