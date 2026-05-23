//! Tauri IPC commands. Each #[tauri::command] is a thin shim around a plain
//! `*_inner` function (no Tauri state) so unit tests can exercise the logic
//! without spinning the Tauri runtime.

use crate::app_mode::AppMode;
use crate::diff::{PropertyChange, diff_properties};
use crate::graph_diff::{GraphDiff, ThreeWayGraphDiff, diff_graphs_inner, diff_graphs_three_way_inner};
use crate::merge;
use crate::schema::AssetSnapshot;
use crate::sidecar::{Sidecar, SidecarConfig};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Returned to the frontend at startup so the React app knows whether to
/// open the standalone list or the focused merge view.
pub fn get_app_mode_inner(mode: &AppMode) -> AppMode {
    mode.clone()
}

#[tauri::command]
pub fn get_app_mode(state: tauri::State<'_, AppMode>) -> AppMode {
    get_app_mode_inner(&state)
}

pub fn diff_snapshots_inner(ours: &AssetSnapshot, theirs: &AssetSnapshot) -> Vec<PropertyChange> {
    diff_properties(&ours.asset.properties, &theirs.asset.properties)
}

#[tauri::command]
pub fn diff_snapshots(ours: AssetSnapshot, theirs: AssetSnapshot) -> Vec<PropertyChange> {
    diff_snapshots_inner(&ours, &theirs)
}

pub fn diff_graphs_ipc_inner(ours: &AssetSnapshot, theirs: &AssetSnapshot) -> Vec<GraphDiff> {
    let ours_graphs = ours.asset.graphs.clone().unwrap_or_default();
    let theirs_graphs = theirs.asset.graphs.clone().unwrap_or_default();
    diff_graphs_inner(&ours_graphs, &theirs_graphs)
}

#[tauri::command]
pub fn diff_graphs(ours: AssetSnapshot, theirs: AssetSnapshot) -> Vec<GraphDiff> {
    diff_graphs_ipc_inner(&ours, &theirs)
}

pub fn diff_graphs_three_way_ipc_inner(
    ancestor: &AssetSnapshot,
    ours: &AssetSnapshot,
    theirs: &AssetSnapshot,
) -> Vec<ThreeWayGraphDiff> {
    let anc_graphs = ancestor.asset.graphs.clone().unwrap_or_default();
    let ours_graphs = ours.asset.graphs.clone().unwrap_or_default();
    let theirs_graphs = theirs.asset.graphs.clone().unwrap_or_default();
    diff_graphs_three_way_inner(&anc_graphs, &ours_graphs, &theirs_graphs)
}

#[tauri::command]
pub fn diff_graphs_three_way(
    ancestor: AssetSnapshot,
    ours: AssetSnapshot,
    theirs: AssetSnapshot,
) -> Vec<ThreeWayGraphDiff> {
    diff_graphs_three_way_ipc_inner(&ancestor, &ours, &theirs)
}

pub fn apply_resolution_inner(
    resolution: &str,
    ours: &Path,
    theirs: &Path,
    dest: &Path,
) -> Result<(), String> {
    let res: merge::Resolution = resolution
        .parse::<merge::Resolution>()
        .map_err(|e| e.to_string())?;
    merge::apply_resolution(res, ours, theirs, dest).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn apply_resolution(
    resolution: String,
    ours: String,
    theirs: String,
    dest: String,
) -> Result<(), String> {
    apply_resolution_inner(
        &resolution,
        Path::new(&ours),
        Path::new(&theirs),
        Path::new(&dest),
    )
}

/// Internal: send `merge` JSON-RPC to the commandlet and copy the resulting
/// temp .uasset over `dest`. Pure function over a `Sidecar` so we can swap
/// in the mock for tests.
pub fn apply_graph_merge_inner(
    sidecar: &Sidecar,
    ancestor_path: &Path,
    dest: &Path,
    merged_graphs: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let abs = std::fs::canonicalize(ancestor_path)
        .map_err(|e| format!("canonicalise {}: {}", ancestor_path.display(), e))?;
    let ancestor_str = abs.to_string_lossy().replace('\\', "/");

    let mut graphs_json = serde_json::Map::new();
    for (k, v) in merged_graphs {
        graphs_json.insert(k.clone(), serde_json::Value::String(v.clone()));
    }
    let req = serde_json::json!({
        "id": 1,
        "cmd": "merge",
        "path": ancestor_str,
        "mergedGraphs": serde_json::Value::Object(graphs_json),
    });
    let responses = sidecar.run_batch(&[req]).map_err(|e| e.to_string())?;
    let resp = responses
        .into_iter()
        .find(|r| r.get("id").and_then(|i| i.as_u64()) == Some(1))
        .ok_or_else(|| "no id=1 response from sidecar".to_string())?;
    if !resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        let err = resp.get("error").and_then(|v| v.as_str()).unwrap_or("unknown error");
        return Err(format!("commandlet merge failed: {}", err));
    }
    let merged_path = resp
        .get("mergedPath")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "commandlet response missing 'mergedPath'".to_string())?;

    let merged_pb = PathBuf::from(merged_path);
    merge::apply_merged_file(&merged_pb, dest).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&merged_pb);
    Ok(())
}

#[tauri::command]
pub fn apply_graph_merge(
    ancestor_path: String,
    dest_path: String,
    merged_graphs: HashMap<String, String>,
    sidecar_override: Option<String>,
    host_project_override: Option<String>,
) -> Result<(), String> {
    let exe = sidecar_override
        .map(PathBuf::from)
        .unwrap_or_else(default_sidecar);
    let host_project = host_project_override
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("ue-host/HostProject.uproject"));

    let args = if exe.to_string_lossy().to_lowercase().contains("unrealeditor") {
        vec![
            host_project.display().to_string(),
            "-run=MergeBinariesExport".to_string(),
            "-stdio".to_string(),
            "-nullrhi".to_string(),
            "-unattended".to_string(),
            "-NoCrashReports".to_string(),
        ]
    } else {
        Vec::new()
    };
    let log_redirect = if exe.to_string_lossy().to_lowercase().contains("unrealeditor") {
        Some(std::env::temp_dir().join(format!(
            "unreal-merge-ipc-{}.log",
            std::process::id()
        )))
    } else {
        None
    };

    let sidecar = Sidecar::new(SidecarConfig {
        executable: exe,
        args,
        prepend_warmup: true,
        log_redirect,
    });

    apply_graph_merge_inner(&sidecar, Path::new(&ancestor_path), Path::new(&dest_path), &merged_graphs)
}

#[tauri::command]
pub fn export_asset(
    path: String,
    sidecar_override: Option<String>,
    host_project_override: Option<String>,
) -> Result<AssetSnapshot, String> {
    let exe = sidecar_override
        .map(PathBuf::from)
        .unwrap_or_else(default_sidecar);
    let host_project = host_project_override
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("ue-host/HostProject.uproject"));

    let args = if exe.to_string_lossy().to_lowercase().contains("unrealeditor") {
        vec![
            host_project.display().to_string(),
            "-run=MergeBinariesExport".to_string(),
            "-stdio".to_string(),
            "-nullrhi".to_string(),
            "-unattended".to_string(),
            "-NoCrashReports".to_string(),
        ]
    } else {
        Vec::new()
    };
    let log_redirect = if exe.to_string_lossy().to_lowercase().contains("unrealeditor") {
        Some(std::env::temp_dir().join(format!(
            "unreal-merge-ipc-{}.log",
            std::process::id()
        )))
    } else {
        None
    };

    let sidecar = Sidecar::new(SidecarConfig {
        executable: exe,
        args,
        prepend_warmup: true,
        log_redirect,
    });

    let abs = std::fs::canonicalize(&path).map_err(|e| format!("canonicalise {}: {}", path, e))?;
    let path_str = abs.to_string_lossy().replace('\\', "/");
    let requests = vec![serde_json::json!({"id": 1, "cmd": "export", "path": path_str})];

    let responses = sidecar.run_batch(&requests).map_err(|e| e.to_string())?;
    let response = responses
        .into_iter()
        .find(|r| r.get("id").and_then(|i| i.as_u64()) == Some(1))
        .ok_or_else(|| "no id=1 response from sidecar".to_string())?;
    let snap: AssetSnapshot =
        serde_json::from_value(response).map_err(|e| format!("parse snapshot: {}", e))?;
    if !snap.ok {
        return Err("commandlet reported ok=false".to_string());
    }
    Ok(snap)
}

fn default_sidecar() -> PathBuf {
    if let Ok(val) = std::env::var("UNREAL_MERGE_SIDECAR") {
        return PathBuf::from(val);
    }
    // In debug builds, prefer the mock sidecar if it lives next to the binary.
    // This means `pnpm tauri dev` works out of the box without a real UE install.
    // Override with UNREAL_MERGE_SIDECAR to test against real UE in debug mode.
    #[cfg(debug_assertions)]
    if let Ok(exe) = std::env::current_exe() {
        let mock = exe.with_file_name("mock_ue_sidecar.exe");
        if mock.exists() {
            return mock;
        }
    }
    PathBuf::from(r"C:\Program Files\Epic Games\UE_5.6\Engine\Binaries\Win64\UnrealEditor.exe")
}

#[tauri::command]
pub fn close_with_exit(window: tauri::Window, code: i32) {
    // Hide window first so the close feels instant; then exit with the
    // exit code Git expects (0 = resolved, 1 = abort).
    let _ = window.hide();
    std::process::exit(code);
}
