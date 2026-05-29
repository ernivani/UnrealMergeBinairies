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
use std::sync::atomic::{AtomicU64, Ordering};

/// Git's merge driver hands us extension-less temp blobs (e.g. `.merge_file_x9Q2`).
/// The UE commandlet's package loader requires a `.uasset`/`.umap` file on disk.
/// Returns true when `path` must be copied to a `.uasset` temp before loading.
fn needs_staging(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => {
            let ext = ext.to_ascii_lowercase();
            ext != "uasset" && ext != "umap"
        }
        None => true,
    }
}

/// Copy `src` to a uniquely-named `.uasset` file in the temp dir so the
/// commandlet can load it. Caller is responsible for deleting the result.
fn stage_as_uasset(src: &Path) -> std::io::Result<PathBuf> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let raw = src.file_name().and_then(|s| s.to_str()).unwrap_or("asset");
    let clean: String = raw
        .trim_start_matches('.')
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    let clean = if clean.is_empty() { "asset".to_string() } else { clean };
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dest = std::env::temp_dir().join(format!(
        "unreal_merge_stage_{}_{}_{}.uasset",
        std::process::id(),
        n,
        clean
    ));
    std::fs::copy(src, &dest)?;
    Ok(dest)
}

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
/// temp .uasset over `dest`. `target_path` is the real asset inside the open
/// project (loaded by its /Game name so references resolve and the saved asset
/// keeps the correct internal package name). Pure function over a `Sidecar` so
/// tests can swap in the mock.
pub fn apply_graph_merge_inner(
    sidecar: &Sidecar,
    target_path: &Path,
    dest: &Path,
    merged_graphs: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let abs = std::fs::canonicalize(target_path)
        .map_err(|e| format!("canonicalise {}: {}", target_path.display(), e))?;
    let target_str = abs.to_string_lossy().replace('\\', "/");
    // Strip Windows' \\?\ extended-length prefix (becomes //?/ after the slash
    // swap) — UE's FPackageName::TryConvertFilenameToLongPackageName doesn't
    // recognise it and would fail to match the mounted Content dir.
    let target_str = target_str
        .strip_prefix("//?/")
        .map(str::to_string)
        .unwrap_or(target_str);

    let mut graphs_json = serde_json::Map::new();
    for (k, v) in merged_graphs {
        graphs_json.insert(k.clone(), serde_json::Value::String(v.clone()));
    }
    let req = serde_json::json!({
        "id": 1,
        "cmd": "merge",
        "targetPath": target_str,
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
    target_path: String,
    dest_path: String,
    merged_graphs: HashMap<String, String>,
    sidecar_override: Option<String>,
    host_project_override: Option<String>,
) -> Result<(), String> {
    let exe = sidecar_override
        .map(PathBuf::from)
        .unwrap_or_else(default_sidecar);
    // The target is the real asset inside the game project; resolve the owning
    // .uproject by walking up from it.
    let host_project = resolve_host_project(Path::new(&target_path), host_project_override);

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

    apply_graph_merge_inner(&sidecar, Path::new(&target_path), Path::new(&dest_path), &merged_graphs)
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
    let host_project = resolve_host_project(Path::new(&path), host_project_override);

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

    // Stage extension-less inputs (git temp blobs) into a real .uasset file.
    let src = Path::new(&path);
    let staged = if needs_staging(src) {
        Some(stage_as_uasset(src).map_err(|e| format!("stage {}: {}", path, e))?)
    } else {
        None
    };
    let load_path = staged.as_deref().unwrap_or(src);

    let result = (|| {
        let abs = std::fs::canonicalize(load_path)
            .map_err(|e| format!("canonicalise {}: {}", load_path.display(), e))?;
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
    })();

    if let Some(s) = staged {
        let _ = std::fs::remove_file(s);
    }
    result
}

/// Walk up from `start` looking for the nearest `*.uproject`. Returns the first
/// one found. `start` may be a file (we begin at its parent) or a directory.
fn find_uproject_upwards(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_dir() {
        Some(start.to_path_buf())
    } else {
        start.parent().map(|p| p.to_path_buf())
    };
    for _ in 0..40 {
        let d = dir.as_ref()?;
        if let Ok(entries) = std::fs::read_dir(d) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().map(|e| e.eq_ignore_ascii_case("uproject")).unwrap_or(false) {
                    return Some(p);
                }
            }
        }
        dir = d.parent().map(|p| p.to_path_buf());
    }
    None
}

/// Resolve which UE project to open for the sidecar. To make Blueprint diffs
/// accurate, we open the project that OWNS the conflicted asset so its C++
/// modules, content, and referenced types all resolve. Resolution order:
///   1. explicit override (CLI/IPC param)
///   2. the game `.uproject` found by walking up from the asset path
///   3. UNREAL_MERGE_HOST_PROJECT env var (manual escape hatch)
///   4. the bundled ue-host project (fallback — degraded reference resolution)
fn resolve_host_project(near: &Path, override_opt: Option<String>) -> PathBuf {
    if let Some(o) = override_opt {
        return PathBuf::from(o);
    }
    // Prefer the canonicalised path so the upward walk uses absolute dirs.
    let canon = std::fs::canonicalize(near).ok();
    let probe = canon.as_deref().unwrap_or(near);
    if let Some(up) = find_uproject_upwards(probe) {
        return up;
    }
    if let Ok(val) = std::env::var("UNREAL_MERGE_HOST_PROJECT") {
        return PathBuf::from(val);
    }
    bundled_host_project()
}

/// The bundled minimal host project shipped alongside this tool. Used only as a
/// last resort when the asset's own project can't be located.
fn bundled_host_project() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..6 {
            if let Some(d) = &dir {
                let candidate = d.join("ue-host").join("HostProject.uproject");
                if candidate.exists() {
                    return candidate;
                }
                dir = d.parent().map(|p| p.to_path_buf());
            }
        }
    }
    PathBuf::from("ue-host/HostProject.uproject")
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

#[cfg(test)]
mod staging_tests {
    use super::*;

    #[test]
    fn needs_staging_detects_extensions() {
        assert!(needs_staging(Path::new(".merge_file_x9Q2")));
        assert!(needs_staging(Path::new("/tmp/.merge_file_ABC")));
        assert!(needs_staging(Path::new("blob")));
        assert!(!needs_staging(Path::new("BP_Foo.uasset")));
        assert!(!needs_staging(Path::new("Map.umap")));
        assert!(!needs_staging(Path::new("/a/b/C.UASSET"))); // case-insensitive
    }

    #[test]
    fn stage_as_uasset_copies_with_uasset_extension() {
        let tmp = std::env::temp_dir();
        let src = tmp.join(format!("unreal_merge_srctest_{}.merge_blob", std::process::id()));
        std::fs::write(&src, b"hello-bytes").unwrap();

        let staged = stage_as_uasset(&src).expect("stage");
        assert_eq!(
            staged.extension().and_then(|e| e.to_str()),
            Some("uasset"),
            "staged file must end in .uasset"
        );
        assert_eq!(std::fs::read(&staged).unwrap(), b"hello-bytes", "contents preserved");

        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&staged);
    }
}
