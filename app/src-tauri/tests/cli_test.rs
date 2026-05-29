use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn help_lists_subcommands() {
    Command::cargo_bin("unreal-merge")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("install"))
        .stdout(contains("uninstall"))
        .stdout(contains("scan"))
        .stdout(contains("export"));
}

#[test]
fn export_against_nonexistent_file_exits_nonzero() {
    // Without a sidecar reachable, we still want exit nonzero (not panic).
    let output = Command::cargo_bin("unreal-merge")
        .unwrap()
        .args([
            "export",
            "--sidecar",
            "C:/does/not/exist.exe",
            "C:/also/missing.uasset",
        ])
        .unwrap_err();
    let _ = output; // assert_cmd's `.unwrap_err` means the command exited nonzero - that's enough.
}
