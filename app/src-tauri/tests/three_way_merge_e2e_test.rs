//! End-to-end test: drive `apply_graph_merge_inner` against the mock sidecar
//! and verify the merged file lands at `dest_path`.

use std::collections::HashMap;
use std::path::PathBuf;
use unreal_merge::ipc::apply_graph_merge_inner;
use unreal_merge::sidecar::{Sidecar, SidecarConfig};

fn mock_sidecar_path() -> PathBuf {
    // Built alongside the integration test binary.
    let mut exe = std::env::current_exe().expect("current_exe");
    // current_exe is something like target/debug/deps/three_way_merge_e2e_test-<hash>.exe
    exe.pop(); // deps
    exe.pop(); // debug
    let name = if cfg!(windows) { "mock_ue_sidecar.exe" } else { "mock_ue_sidecar" };
    exe.join(name)
}

#[test]
fn apply_graph_merge_writes_dest_via_mock() {
    let mock = mock_sidecar_path();
    assert!(mock.exists(), "mock sidecar not built at {}; run `cargo build --bin mock_ue_sidecar` first", mock.display());

    let tmp = std::env::temp_dir();
    let pid = std::process::id();
    let ancestor = tmp.join(format!("unreal-merge-test-anc-{}.uasset", pid));
    let dest = tmp.join(format!("unreal-merge-test-dest-{}.uasset", pid));
    std::fs::write(&ancestor, b"ancestor placeholder").unwrap();
    std::fs::write(&dest, b"dest placeholder").unwrap();

    let sidecar = Sidecar::new(SidecarConfig {
        executable: mock,
        args: Vec::new(),
        prepend_warmup: true,
        log_redirect: None,
    });

    let mut merged_graphs = HashMap::new();
    merged_graphs.insert(
        "EventGraph".to_string(),
        "Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name=\"Test\"\n   NodeGuid=00000000000000000000000000000099\nEnd Object\n".to_string(),
    );

    apply_graph_merge_inner(&sidecar, &ancestor, &dest, &merged_graphs)
        .expect("apply_graph_merge_inner");

    let written = std::fs::read_to_string(&dest).expect("read dest");
    assert!(written.contains("// graph: EventGraph"), "dest text: {}", written);
    assert!(written.contains("NodeGuid=00000000000000000000000000000099"), "dest text: {}", written);

    let _ = std::fs::remove_file(&ancestor);
    let _ = std::fs::remove_file(&dest);
}
