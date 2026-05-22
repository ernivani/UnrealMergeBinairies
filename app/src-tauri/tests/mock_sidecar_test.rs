use assert_cmd::Command;
use pretty_assertions::assert_eq;

#[test]
fn mock_handles_ping_and_quit() {
    let mut cmd = Command::cargo_bin("mock_ue_sidecar").unwrap();
    cmd.write_stdin(
        "{\"id\":1,\"cmd\":\"ping\"}\n{\"id\":2,\"cmd\":\"quit\"}\n",
    );
    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();

    // Expect: noise line then two JSON responses.
    let json_lines: Vec<&str> = stdout
        .lines()
        .filter(|l| l.starts_with('{'))
        .collect();
    assert_eq!(json_lines.len(), 2);
    assert!(json_lines[0].contains("\"pong\":\"mock_ue_sidecar\""));
    assert!(json_lines[1].contains("\"id\":2"));
}

#[test]
fn mock_export_returns_canned_snapshot() {
    let mut cmd = Command::cargo_bin("mock_ue_sidecar").unwrap();
    cmd.write_stdin(
        "{\"id\":1,\"cmd\":\"export\",\"path\":\"X:/anything.uasset\"}\n{\"id\":2,\"cmd\":\"quit\"}\n",
    );
    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();

    let resp: serde_json::Value = stdout
        .lines()
        .filter(|l| l.starts_with('{'))
        .find_map(|l| serde_json::from_str(l).ok())
        .expect("at least one JSON response");
    assert_eq!(resp["ok"], true);
    assert_eq!(resp["asset"]["class"], "Blueprint");
    assert_eq!(resp["package"]["fileVersionUE5"], 1017);
}
