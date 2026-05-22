//! Tauri IPC commands. Each #[tauri::command] is a thin shim around a plain
//! `*_inner` function (no Tauri state) so unit tests can exercise the logic
//! without spinning the Tauri runtime.

use crate::app_mode::AppMode;
use crate::diff::{PropertyChange, diff_properties};
use crate::merge;
use crate::schema::AssetSnapshot;
use crate::sidecar::{Sidecar, SidecarConfig};
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
    PathBuf::from(r"C:\Program Files\Epic Games\UE_5.6\Engine\Binaries\Win64\UnrealEditor.exe")
}

#[tauri::command]
pub fn close_with_exit(window: tauri::Window, code: i32) {
    // Hide window first so the close feels instant; then exit with the
    // exit code Git expects (0 = resolved, 1 = abort).
    let _ = window.hide();
    std::process::exit(code);
}
