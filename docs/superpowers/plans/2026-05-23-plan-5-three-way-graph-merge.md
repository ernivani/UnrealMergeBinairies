# Plan 5 — Three-Way Graph Merge ("Take Both") Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a VS Code-style "Take Both" merge path for Blueprint `.uasset` files — non-conflicting graph-node changes from both sides are auto-accepted; conflicts get a per-node Ours / Theirs / Skip picker; the merged result is rewritten by a new UE commandlet `merge` op and written to the working tree.

**Architecture:** Rust adds a `diff_graphs_three_way` IPC that returns per-GUID `ThreeWayNodeStatus` across ancestor / ours / theirs. Frontend `GraphView` switches into 3-way mode when an `ancestorPath` is present, renders per-node overlay badges, lets the user resolve conflicts, then submits a `merged_graphs` map to a new `apply_graph_merge` IPC. That IPC sends a `merge` JSON-RPC to the commandlet which duplicates the ancestor `.uasset`, replaces nodes per graph via `FEdGraphUtilities::ImportNodesFromText`, calls `SavePackage`, returns a temp path, and the Rust side copies it to `dest`. The mock sidecar implements `merge` by writing a plain-text file so dev mode stays useful.

**Tech Stack:** Rust (serde, std), Tauri 2 IPC, React 18 + TS, CSS Modules, UE 5.6 C++ (`FEdGraphUtilities::ImportNodesFromText`, `StaticDuplicateObject`, `UPackage::SavePackage`).

---

## File Map

| File | Change |
|---|---|
| `app/src-tauri/src/graph_diff.rs` | Add `ThreeWayNodeStatus`, `ThreeWayGraphDiff`, `diff_graphs_three_way_inner` + tests |
| `app/src-tauri/src/merge.rs` | Extract `apply_merged_file` helper; reuse in `apply_resolution` |
| `app/src-tauri/src/ipc.rs` | Add `diff_graphs_three_way` + `apply_graph_merge` commands |
| `app/src-tauri/src/lib.rs` | Re-export new types |
| `app/src-tauri/src/main.rs` | Register new commands in `generate_handler!` |
| `app/src-tauri/src/bin/mock_ue_sidecar.rs` | Add `EVENT_GRAPH_ANCESTOR`, `merge` cmd, ancestor-path detection |
| `app/src-tauri/tests/three_way_merge_e2e_test.rs` | **New** end-to-end mock-backed test |
| `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeApplier.h` | **New** |
| `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeApplier.cpp` | **New** |
| `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeBinariesExportCommandlet.cpp` | Register `merge` handler |
| `app/src/types.ts` | Add `ThreeWayNodeStatus`, `ThreeWayGraphDiff`, `MergeSide` |
| `app/src/ipc.ts` | Add `diffGraphsThreeWay`, `applyGraphMerge` |
| `app/src/mergeGraphs.ts` | **New** — `parseNodeBlobs` + `buildMergedGraphs` |
| `app/src/graphDiff.ts` | Add `applyThreeWayOverlay` |
| `app/src/styles.css` | Add `.uem-three-way-*` classes (conflict magenta, dim opacity) |
| `app/src/views/GraphView.tsx` | 3-way mode: per-node selection state + picker overlay |
| `app/src/views/GraphView.module.css` | Picker overlay styles |
| `app/src/views/GraphPane.tsx` | Pass selections through to overlay |
| `app/src/views/Resolve.tsx` | Add optional `onTakeBoth` button |
| `app/src/views/Resolve.module.css` | "both" button styles |
| `app/src/views/Diff.tsx` | Accept `ancestorPath`; fetch ancestor; compute 3-way; wire Take Both |
| `app/src/App.tsx` | Pass `mode.ancestor` to `Diff` |
| `app/src/views/BlueprintTest.tsx` | Add ancestor snapshot; render 3-way view |

---

### Task 1: Rust — ThreeWayNodeStatus + ThreeWayGraphDiff + tests

**Files:**
- Modify: `app/src-tauri/src/graph_diff.rs`

- [ ] **Step 1: Add the new types and tests above the existing `#[cfg(test)] mod tests`**

After the existing `GraphDiff` struct (line ~26) in `app/src-tauri/src/graph_diff.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThreeWayNodeStatus {
    Unchanged,
    ModifiedInOurs,
    ModifiedInTheirs,
    ModifiedInBoth,
    AddedInOurs,
    AddedInTheirs,
    AddedInBoth,
    AddedInBothConflict,
    RemovedInOurs,
    RemovedInTheirs,
    RemovedInBoth,
    ModifyDeleteConflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeWayGraphDiff {
    pub name: String,
    pub only_in_ours: bool,
    pub only_in_theirs: bool,
    pub only_in_ancestor: bool,
    pub node_statuses: HashMap<String, ThreeWayNodeStatus>,
}

/// Compute per-GUID three-way status. The ancestor map represents the
/// merge base (git's %O); `ours` and `theirs` are %A and %B.
///
/// When a graph is missing from a side, its node set is empty for that side.
pub fn diff_graphs_three_way_inner(
    ancestor_graphs: &HashMap<String, String>,
    ours_graphs: &HashMap<String, String>,
    theirs_graphs: &HashMap<String, String>,
) -> Vec<ThreeWayGraphDiff> {
    let mut all_names: std::collections::BTreeSet<String> = Default::default();
    all_names.extend(ancestor_graphs.keys().cloned());
    all_names.extend(ours_graphs.keys().cloned());
    all_names.extend(theirs_graphs.keys().cloned());

    let mut result = Vec::new();
    for name in all_names {
        let anc_text = ancestor_graphs.get(&name);
        let our_text = ours_graphs.get(&name);
        let thr_text = theirs_graphs.get(&name);

        let only_in_ancestor = anc_text.is_some() && our_text.is_none() && thr_text.is_none();
        let only_in_ours = our_text.is_some() && anc_text.is_none() && thr_text.is_none();
        let only_in_theirs = thr_text.is_some() && anc_text.is_none() && our_text.is_none();

        let anc_nodes = anc_text.map(|t| parse_node_blobs(t)).unwrap_or_default();
        let our_nodes = our_text.map(|t| parse_node_blobs(t)).unwrap_or_default();
        let thr_nodes = thr_text.map(|t| parse_node_blobs(t)).unwrap_or_default();

        let mut all_guids: std::collections::BTreeSet<&String> = Default::default();
        all_guids.extend(anc_nodes.keys());
        all_guids.extend(our_nodes.keys());
        all_guids.extend(thr_nodes.keys());

        let mut node_statuses = HashMap::new();
        for guid in all_guids {
            let a = anc_nodes.get(guid);
            let o = our_nodes.get(guid);
            let t = thr_nodes.get(guid);

            let status = match (a, o, t) {
                // present nowhere — unreachable but cheap to handle
                (None, None, None) => continue,
                // only in ancestor
                (Some(_), None, None) => ThreeWayNodeStatus::RemovedInBoth,
                // added by one side
                (None, Some(_), None) => ThreeWayNodeStatus::AddedInOurs,
                (None, None, Some(_)) => ThreeWayNodeStatus::AddedInTheirs,
                // added by both
                (None, Some(o_b), Some(t_b)) => {
                    if o_b == t_b {
                        ThreeWayNodeStatus::AddedInBoth
                    } else {
                        ThreeWayNodeStatus::AddedInBothConflict
                    }
                }
                // modify/delete
                (Some(_), Some(_), None) => {
                    let o_b = o.unwrap();
                    let a_b = a.unwrap();
                    if o_b == a_b {
                        // ours unchanged, theirs deleted → just removed in theirs
                        ThreeWayNodeStatus::RemovedInTheirs
                    } else {
                        ThreeWayNodeStatus::ModifyDeleteConflict
                    }
                }
                (Some(_), None, Some(_)) => {
                    let t_b = t.unwrap();
                    let a_b = a.unwrap();
                    if t_b == a_b {
                        ThreeWayNodeStatus::RemovedInOurs
                    } else {
                        ThreeWayNodeStatus::ModifyDeleteConflict
                    }
                }
                // present everywhere
                (Some(a_b), Some(o_b), Some(t_b)) => {
                    let o_eq_a = o_b == a_b;
                    let t_eq_a = t_b == a_b;
                    let o_eq_t = o_b == t_b;
                    if o_eq_a && t_eq_a {
                        ThreeWayNodeStatus::Unchanged
                    } else if o_eq_a {
                        ThreeWayNodeStatus::ModifiedInTheirs
                    } else if t_eq_a {
                        ThreeWayNodeStatus::ModifiedInOurs
                    } else if o_eq_t {
                        // Both sides made the same modification — pick either side, no conflict.
                        ThreeWayNodeStatus::ModifiedInOurs
                    } else {
                        ThreeWayNodeStatus::ModifiedInBoth
                    }
                }
            };
            node_statuses.insert(guid.clone(), status);
        }

        result.push(ThreeWayGraphDiff {
            name,
            only_in_ours,
            only_in_theirs,
            only_in_ancestor,
            node_statuses,
        });
    }
    result
}
```

- [ ] **Step 2: Add unit tests at the bottom of the existing `mod tests` block**

Inside `app/src-tauri/src/graph_diff.rs`'s `#[cfg(test)] mod tests`, before the closing `}`, append:

```rust
    const NODE_A_V2: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name=\"K2Node_Event_0\"
   NodeGuid=AAAAAAAA000000000000000000000001
   NodePosX=300
End Object
";

    fn three_way_status(
        anc: &[(&str, &str)],
        ours: &[(&str, &str)],
        theirs: &[(&str, &str)],
        guid: &str,
    ) -> Option<ThreeWayNodeStatus> {
        let diffs = diff_graphs_three_way_inner(
            &make_graphs(anc), &make_graphs(ours), &make_graphs(theirs),
        );
        diffs.iter().find(|d| d.name == "EventGraph")
            .and_then(|d| d.node_statuses.get(guid).cloned())
    }

    #[test]
    fn three_way_unchanged() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)], &[("EventGraph", NODE_A)], &[("EventGraph", NODE_A)],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::Unchanged));
    }

    #[test]
    fn three_way_modified_in_ours() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", NODE_A_CHANGED)],
            &[("EventGraph", NODE_A)],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::ModifiedInOurs));
    }

    #[test]
    fn three_way_modified_in_theirs() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", NODE_A)],
            &[("EventGraph", NODE_A_CHANGED)],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::ModifiedInTheirs));
    }

    #[test]
    fn three_way_modified_in_both_same_change_is_not_conflict() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", NODE_A_CHANGED)],
            &[("EventGraph", NODE_A_CHANGED)],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::ModifiedInOurs));
    }

    #[test]
    fn three_way_modified_in_both_conflict() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", NODE_A_CHANGED)],
            &[("EventGraph", NODE_A_V2)],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::ModifiedInBoth));
    }

    #[test]
    fn three_way_removed_in_ours() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", "")],
            &[("EventGraph", NODE_A)],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::RemovedInOurs));
    }

    #[test]
    fn three_way_removed_in_theirs() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", NODE_A)],
            &[("EventGraph", "")],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::RemovedInTheirs));
    }

    #[test]
    fn three_way_removed_in_both() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", "")],
            &[("EventGraph", "")],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::RemovedInBoth));
    }

    #[test]
    fn three_way_modify_delete_conflict_ours_kept() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", NODE_A_CHANGED)],
            &[("EventGraph", "")],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::ModifyDeleteConflict));
    }

    #[test]
    fn three_way_modify_delete_conflict_theirs_kept() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", "")],
            &[("EventGraph", NODE_A_CHANGED)],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::ModifyDeleteConflict));
    }

    #[test]
    fn three_way_added_in_ours() {
        let s = three_way_status(
            &[("EventGraph", "")],
            &[("EventGraph", NODE_B)],
            &[("EventGraph", "")],
            "BBBBBBBB000000000000000000000002",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::AddedInOurs));
    }

    #[test]
    fn three_way_added_in_theirs() {
        let s = three_way_status(
            &[("EventGraph", "")],
            &[("EventGraph", "")],
            &[("EventGraph", NODE_B)],
            "BBBBBBBB000000000000000000000002",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::AddedInTheirs));
    }

    #[test]
    fn three_way_added_in_both_identical() {
        let s = three_way_status(
            &[("EventGraph", "")],
            &[("EventGraph", NODE_B)],
            &[("EventGraph", NODE_B)],
            "BBBBBBBB000000000000000000000002",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::AddedInBoth));
    }

    #[test]
    fn three_way_added_in_both_conflict() {
        // Two different node blobs that share the same GUID (rare in practice
        // but the algorithm should flag them as a conflict).
        let other_b = "Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_0\"
   NodeGuid=BBBBBBBB000000000000000000000002
   NodePosX=999
End Object
";
        let s = three_way_status(
            &[("EventGraph", "")],
            &[("EventGraph", NODE_B)],
            &[("EventGraph", other_b)],
            "BBBBBBBB000000000000000000000002",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::AddedInBothConflict));
    }

    #[test]
    fn three_way_graph_only_in_ancestor_yields_removed_in_both() {
        let diffs = diff_graphs_three_way_inner(
            &make_graphs(&[("DeadGraph", NODE_A)]),
            &make_graphs(&[]),
            &make_graphs(&[]),
        );
        let dead = diffs.iter().find(|d| d.name == "DeadGraph").unwrap();
        assert!(dead.only_in_ancestor);
        assert_eq!(
            dead.node_statuses.get("AAAAAAAA000000000000000000000001"),
            Some(&ThreeWayNodeStatus::RemovedInBoth),
        );
    }
```

- [ ] **Step 3: Run the new tests**

From `app/src-tauri/`:
```
cargo test graph_diff::tests::three_way
```
Expected: all `three_way_*` tests pass (14 new + 7 pre-existing graph_diff tests).

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/src/graph_diff.rs
git commit -m "feat(rust): three-way graph diff — ThreeWayNodeStatus + truth-table tests"
```

---

### Task 2: Rust — refactor `apply_resolution` to expose `apply_merged_file`

**Files:**
- Modify: `app/src-tauri/src/merge.rs`

This factor-out lets the new `apply_graph_merge` IPC reuse the read-only-bit dance.

- [ ] **Step 1: Replace the body of `apply_resolution` and add `apply_merged_file`**

Replace lines 29-57 of `app/src-tauri/src/merge.rs` with:

```rust
/// Copy `ours` or `theirs` over `dest`. Returns Err on Abort.
pub fn apply_resolution(res: Resolution, ours: &Path, theirs: &Path, dest: &Path) -> Result<()> {
    let source = match res {
        Resolution::Ours => ours,
        Resolution::Theirs => theirs,
        Resolution::Abort => bail!("aborted by user; conflict left in place"),
    };
    apply_merged_file(source, dest)
}

/// Copy `source` over `dest`, preserving the read-only bit if `dest` had it
/// set (e.g. for LFS-locked files — spec §8 case 8a).
pub fn apply_merged_file(source: &Path, dest: &Path) -> Result<()> {
    let dest_meta = std::fs::metadata(dest).ok();
    let was_readonly = dest_meta
        .as_ref()
        .map(|m| m.permissions().readonly())
        .unwrap_or(false);
    if was_readonly {
        let mut perms = dest_meta.unwrap().permissions();
        perms.set_readonly(false);
        std::fs::set_permissions(dest, perms)?;
    }

    std::fs::copy(source, dest)?;

    if was_readonly {
        let mut perms = std::fs::metadata(dest)?.permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(dest, perms)?;
    }
    Ok(())
}
```

- [ ] **Step 2: Run merge tests**

From `app/src-tauri/`:
```
cargo test merge
```
Expected: existing merge tests still pass.

- [ ] **Step 3: Commit**

```bash
git add app/src-tauri/src/merge.rs
git commit -m "refactor(rust): extract apply_merged_file from apply_resolution"
```

---

### Task 3: Rust — `diff_graphs_three_way` + `apply_graph_merge` IPC commands

**Files:**
- Modify: `app/src-tauri/src/ipc.rs`

- [ ] **Step 1: Update imports at top of `app/src-tauri/src/ipc.rs`**

Change the existing `use crate::graph_diff::...` line to:

```rust
use crate::graph_diff::{GraphDiff, ThreeWayGraphDiff, diff_graphs_inner, diff_graphs_three_way_inner};
```

Add at the top of the file (after the existing `use` block):

```rust
use std::collections::HashMap;
```

- [ ] **Step 2: Add the `diff_graphs_three_way` command after the existing `diff_graphs` command**

Append after the existing `pub fn diff_graphs(...)` function in `ipc.rs`:

```rust
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
```

- [ ] **Step 3: Add `apply_graph_merge` after `apply_resolution`**

Append after the existing `apply_resolution` command in `ipc.rs`:

```rust
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
```

Note: `default_sidecar` is already defined later in the file — no need to redeclare.

- [ ] **Step 4: Run rust check**

From `app/src-tauri/`:
```
cargo check
```
Expected: no errors. (If `Sidecar`/`SidecarConfig`/`merge` are not yet imported at top, add their `use` lines — they should already be present per the existing code.)

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/src/ipc.rs
git commit -m "feat(rust): diff_graphs_three_way + apply_graph_merge IPC commands"
```

---

### Task 4: Rust — wire new commands into lib.rs and main.rs

**Files:**
- Modify: `app/src-tauri/src/lib.rs`
- Modify: `app/src-tauri/src/main.rs`

- [ ] **Step 1: Update re-export in lib.rs**

Replace `pub use graph_diff::{GraphDiff, NodeStatus};` in `app/src-tauri/src/lib.rs` with:

```rust
pub use graph_diff::{GraphDiff, NodeStatus, ThreeWayGraphDiff, ThreeWayNodeStatus};
```

- [ ] **Step 2: Register new commands in main.rs**

In `app/src-tauri/src/main.rs`, replace the `invoke_handler` block (lines 32-39) with:

```rust
                .invoke_handler(tauri::generate_handler![
                    unreal_merge::ipc::get_app_mode,
                    unreal_merge::ipc::diff_snapshots,
                    unreal_merge::ipc::diff_graphs,
                    unreal_merge::ipc::diff_graphs_three_way,
                    unreal_merge::ipc::apply_resolution,
                    unreal_merge::ipc::apply_graph_merge,
                    unreal_merge::ipc::export_asset,
                    unreal_merge::ipc::close_with_exit,
                ])
```

- [ ] **Step 3: Verify it all builds**

From `app/src-tauri/`:
```
cargo test --lib
```
Expected: all lib tests pass (graph_diff three-way tests included).

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/src/lib.rs app/src-tauri/src/main.rs
git commit -m "feat(rust): register three-way IPC commands in Tauri handler + re-export types"
```

---

### Task 5: Mock sidecar — ancestor fixture + `merge` cmd

**Files:**
- Modify: `app/src-tauri/src/bin/mock_ue_sidecar.rs`

- [ ] **Step 1: Add `EVENT_GRAPH_ANCESTOR` const between `EVENT_GRAPH_OURS` and `EVENT_GRAPH_THEIRS`**

Insert before `EVENT_GRAPH_THEIRS` (line ~94) in `app/src-tauri/src/bin/mock_ue_sidecar.rs`:

```rust
// Ancestor = common subset of OURS and THEIRS: BP_Base before either branch changed it.
// Has BeginPlay → SET Health=0.0 → Branch → True PrintString, with Knot from Get Health.
// NO False-branch PrintString (ours added), NO MaxHealth getter (theirs added).
// Pin IDs use the prefix "C0..." so neither side's pin prefix collides.
const EVENT_GRAPH_ANCESTOR: &str = r#"Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name="K2Node_Event_BeginPlay"
   EventReference=(MemberParent=Class'"/Script/Engine.Actor"',MemberName="ReceiveBeginPlay")
   bOverrideFunction=True
   NodeGuid=00000000000000000000000000000001
   NodePosX=-80
   NodePosY=0
   CustomProperties Pin (PinId=C0000000000000000000000000000010,PinName="OutputDelegate",Direction="EGPD_Output",PinType.PinCategory="delegate",PinType.PinSubCategory="MulticastDelegateProperty",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
   CustomProperties Pin (PinId=C0000000000000000000000000000011,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health C0000000000000000000000000000020,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableSet Name="K2Node_VariableSet_Health"
   VariableReference=(MemberName="Health",MemberGuid=AABBCC00000000000000000000000001)
   NodeGuid=00000000000000000000000000000002
   NodePosX=180
   NodePosY=0
   CustomProperties Pin (PinId=C0000000000000000000000000000020,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Event_BeginPlay C0000000000000000000000000000011,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000021,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 C0000000000000000000000000000030,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000022,PinName="Health",Direction="EGPD_Input",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,DefaultValue="0.0",)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_IfThenElse Name="K2Node_IfThenElse_0"
   NodeGuid=00000000000000000000000000000003
   NodePosX=460
   NodePosY=0
   CustomProperties Pin (PinId=C0000000000000000000000000000030,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health C0000000000000000000000000000021,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000031,PinName="Condition",Direction="EGPD_Input",PinType.PinCategory="bool",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 C0000000000000000000000000000061,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000032,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_CallFunction_PrintTrue C0000000000000000000000000000040,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000033,PinName="else",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name="K2Node_CallFunction_PrintTrue"
   FunctionReference=(MemberParent=Class'"/Script/Engine.KismetSystemLibrary"',MemberName="PrintString")
   NodeGuid=00000000000000000000000000000004
   NodePosX=760
   NodePosY=-100
   CustomProperties Pin (PinId=C0000000000000000000000000000040,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 C0000000000000000000000000000032,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000041,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
   CustomProperties Pin (PinId=C0000000000000000000000000000042,PinName="InString",Direction="EGPD_Input",PinType.PinCategory="string",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 C0000000000000000000000000000061,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableGet Name="K2Node_VariableGet_Health"
   VariableReference=(MemberName="Health",MemberGuid=AABBCC00000000000000000000000001)
   NodeGuid=00000000000000000000000000000005
   NodePosX=380
   NodePosY=220
   CustomProperties Pin (PinId=C0000000000000000000000000000050,PinName="Health",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 C0000000000000000000000000000060,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_Knot Name="K2Node_Knot_0"
   NodeGuid=00000000000000000000000000000006
   NodePosX=560
   NodePosY=180
   CustomProperties Pin (PinId=C0000000000000000000000000000060,PinName="InputPin",Direction="EGPD_Input",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableGet_Health C0000000000000000000000000000050,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000061,PinName="OutputPin",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 C0000000000000000000000000000031,K2Node_CallFunction_PrintTrue C0000000000000000000000000000042,),)
End Object
"#;
```

- [ ] **Step 2: Update `handle_export` to recognise ancestor paths**

Replace the existing `handle_export` function (lines ~153-187) with:

```rust
fn handle_export(path: &str, id: Option<&serde_json::Value>) -> serde_json::Value {
    let lower = path.to_lowercase();
    let is_theirs = lower.contains("v2") || lower.contains("theirs");
    let is_ancestor = lower.contains("ancestor") || lower.contains("base") || lower.contains("\\o\\") || lower.contains("/o/");

    let (event_graph, hash, default_health) = if is_ancestor {
        (EVENT_GRAPH_ANCESTOR, "sha1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 50.0)
    } else if is_theirs {
        (EVENT_GRAPH_THEIRS, "sha1:1111111111111111111111111111111111111111", 100.0)
    } else {
        (EVENT_GRAPH_OURS, "sha1:0000000000000000000000000000000000000000", 0.0)
    };

    let mut resp = serde_json::json!({
        "ok": true,
        "path": path,
        "package": {
            "name": "/Game/BP_Base",
            "engineVersion": "5.6.0-mock+++UE5+Release-5.6",
            "fileVersionUE5": 1017,
            "savedHash": hash
        },
        "asset": {
            "class": "Blueprint",
            "parentClass": "/Script/Engine.Actor",
            "name": "BP_Base",
            "properties": [
                {"path": "DefaultHealth", "type": "float", "value": default_health},
                {"path": "MaxHealth", "type": "float", "value": 100.0}
            ],
            "graphs": {
                "EventGraph": event_graph
            }
        }
    });
    if let Some(id_val) = id {
        resp["id"] = id_val.clone();
    }
    resp
}
```

- [ ] **Step 3: Add `handle_merge` and dispatch**

After `handle_export`, add:

```rust
fn handle_merge(req: &serde_json::Value, id: Option<&serde_json::Value>) -> serde_json::Value {
    let merged_graphs = req
        .get("mergedGraphs")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    // Write the concatenation of all merged graph texts to a temp file.
    // The mock doesn't produce a real .uasset — the consumer just copies
    // the bytes over `dest`, which is fine for IPC exercise.
    let mut merged_text = String::new();
    for (name, value) in &merged_graphs {
        merged_text.push_str(&format!("// graph: {}\n", name));
        if let Some(s) = value.as_str() {
            merged_text.push_str(s);
        }
        merged_text.push('\n');
    }

    let temp_dir = std::env::temp_dir();
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let merged_path = temp_dir.join(format!("unreal-merge-mock-{}-{}.uasset", pid, nanos));

    if let Err(e) = std::fs::write(&merged_path, merged_text.as_bytes()) {
        let mut resp = serde_json::json!({"ok": false, "error": format!("write temp: {}", e)});
        if let Some(v) = id { resp["id"] = v.clone(); }
        return resp;
    }

    let mut resp = serde_json::json!({
        "ok": true,
        "mergedPath": merged_path.to_string_lossy().replace('\\', "/"),
    });
    if let Some(v) = id { resp["id"] = v.clone(); }
    resp
}
```

And add a `merge` branch inside the `match req.get("cmd")...` dispatch in `main()` (after the existing `Some("export") => ...` branch):

```rust
            Some("merge") => {
                write_json(&handle_merge(&req, id.as_ref()));
            }
```

- [ ] **Step 4: Build and run mock**

From `app/src-tauri/`:
```
cargo build --bin mock_ue_sidecar
```
Expected: build succeeds.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/src/bin/mock_ue_sidecar.rs
git commit -m "feat(mock-sidecar): add EVENT_GRAPH_ANCESTOR fixture + merge cmd"
```

---

### Task 6: Rust — end-to-end test for `apply_graph_merge` against mock

**Files:**
- Create: `app/src-tauri/tests/three_way_merge_e2e_test.rs`

- [ ] **Step 1: Write the test**

Create `app/src-tauri/tests/three_way_merge_e2e_test.rs`:

```rust
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
```

- [ ] **Step 2: Build & run**

From `app/src-tauri/`:
```
cargo build --bin mock_ue_sidecar
cargo test --test three_way_merge_e2e_test
```
Expected: test passes.

- [ ] **Step 3: Run full test suite to confirm no regressions**

From `app/src-tauri/`:
```
cargo test
```
Expected: all tests pass (existing ~50 + 14 new graph_diff three-way + 1 e2e).

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/tests/three_way_merge_e2e_test.rs
git commit -m "test(rust): e2e — apply_graph_merge writes dest via mock sidecar"
```

---

### Task 7: UE C++ — `MergeApplier` header & implementation

**Files:**
- Create: `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeApplier.h`
- Create: `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeApplier.cpp`

No unit test possible — requires live UE editor. Validation comes from manual end-to-end with a real UE sidecar.

- [ ] **Step 1: Create MergeApplier.h**

```cpp
#pragma once

#include "CoreMinimal.h"
#include "Dom/JsonObject.h"

class FMergeApplier
{
public:
    // Reads request fields:
    //   path: string (ancestor .uasset path on disk)
    //   mergedGraphs: object { graphName: string of UE serialization text }
    //
    // Duplicates the ancestor asset to a temp path, imports merged nodes into
    // each named graph (replacing existing nodes), recompiles, saves package,
    // and writes:
    //   { ok: true, mergedPath: string } on success
    //   { ok: false, error: string } on failure
    static void Apply(const TSharedPtr<FJsonObject>& Req, TSharedRef<FJsonObject>& OutResponse);
};
```

- [ ] **Step 2: Create MergeApplier.cpp**

```cpp
#include "MergeApplier.h"

#include "Engine/Blueprint.h"
#include "EdGraph/EdGraph.h"
#include "EdGraph/EdGraphNode.h"
#include "EdGraphUtilities.h"
#include "HAL/FileManager.h"
#include "Kismet2/BlueprintEditorUtils.h"
#include "Kismet2/KismetEditorUtilities.h"
#include "Misc/FileHelper.h"
#include "Misc/Guid.h"
#include "Misc/Paths.h"
#include "UObject/Package.h"
#include "UObject/SavePackage.h"

namespace
{
    bool DuplicatePackageToTemp(const FString& SrcDiskPath, FString& OutTempDiskPath, UPackage*& OutPackage, FString& OutError)
    {
        if (!FPaths::FileExists(SrcDiskPath))
        {
            OutError = FString::Printf(TEXT("ancestor not found: %s"), *SrcDiskPath);
            return false;
        }

        // Load the source package.
        const FString PackageName = FPackageName::FilenameToLongPackageName(SrcDiskPath);
        UPackage* SrcPackage = LoadPackage(nullptr, *PackageName, LOAD_None);
        if (!SrcPackage)
        {
            OutError = FString::Printf(TEXT("LoadPackage failed for %s"), *PackageName);
            return false;
        }

        // Build a unique temp package name + disk path.
        const FString IntermediateDir = FPaths::ProjectIntermediateDir() / TEXT("UnrealMerge");
        IFileManager::Get().MakeDirectory(*IntermediateDir, /*Tree=*/true);

        const FString UniqueId = FGuid::NewGuid().ToString(EGuidFormats::Short);
        const FString TempPackageName = FString::Printf(TEXT("/Temp/UnrealMerge/Merged_%s"), *UniqueId);
        OutTempDiskPath = IntermediateDir / FString::Printf(TEXT("Merged_%s.uasset"), *UniqueId);

        // Duplicate the package.
        OutPackage = CreatePackage(*TempPackageName);
        if (!OutPackage)
        {
            OutError = TEXT("CreatePackage for temp failed");
            return false;
        }

        // Duplicate every UObject in the source package into the dest package.
        for (TObjectIterator<UObject> It; It; ++It)
        {
            UObject* Obj = *It;
            if (Obj && Obj->GetOutermost() == SrcPackage && !Obj->IsTemplate(RF_ClassDefaultObject))
            {
                StaticDuplicateObject(Obj, OutPackage, Obj->GetFName());
            }
        }

        return true;
    }

    UBlueprint* FindBlueprintInPackage(UPackage* Package)
    {
        for (TObjectIterator<UBlueprint> It; It; ++It)
        {
            if (It->GetOutermost() == Package)
            {
                return *It;
            }
        }
        return nullptr;
    }

    UEdGraph* FindGraphByName(UBlueprint* BP, const FString& Name)
    {
        for (UEdGraph* G : BP->UbergraphPages)   { if (G && G->GetName() == Name) return G; }
        for (UEdGraph* G : BP->FunctionGraphs)   { if (G && G->GetName() == Name) return G; }
        for (UEdGraph* G : BP->MacroGraphs)      { if (G && G->GetName() == Name) return G; }
        return nullptr;
    }

    bool ReplaceGraphNodes(UEdGraph* Graph, const FString& MergedText, FString& OutError)
    {
        // Remove all existing nodes.
        Graph->Modify();
        for (int32 i = Graph->Nodes.Num() - 1; i >= 0; --i)
        {
            UEdGraphNode* N = Graph->Nodes[i];
            if (N)
            {
                Graph->RemoveNode(N);
            }
        }

        // Import merged nodes.
        TSet<UEdGraphNode*> Imported;
        FEdGraphUtilities::ImportNodesFromText(Graph, MergedText, /*out*/ Imported);
        if (Imported.Num() == 0 && !MergedText.IsEmpty())
        {
            OutError = TEXT("ImportNodesFromText produced 0 nodes");
            return false;
        }
        Graph->NotifyGraphChanged();
        return true;
    }
}

void FMergeApplier::Apply(const TSharedPtr<FJsonObject>& Req, TSharedRef<FJsonObject>& OutResponse)
{
    FString AncestorPath;
    if (!Req->TryGetStringField(TEXT("path"), AncestorPath))
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"), TEXT("missing 'path'"));
        return;
    }

    const TSharedPtr<FJsonObject>* MergedGraphsObj = nullptr;
    if (!Req->TryGetObjectField(TEXT("mergedGraphs"), MergedGraphsObj) || !MergedGraphsObj)
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"), TEXT("missing 'mergedGraphs'"));
        return;
    }

    FString TempDiskPath;
    UPackage* TempPackage = nullptr;
    FString Err;
    if (!DuplicatePackageToTemp(AncestorPath, TempDiskPath, TempPackage, Err))
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"), Err);
        return;
    }

    UBlueprint* BP = FindBlueprintInPackage(TempPackage);
    if (!BP)
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"), TEXT("no Blueprint in duplicated package"));
        return;
    }

    for (const auto& Kv : (*MergedGraphsObj)->Values)
    {
        const FString& GraphName = Kv.Key;
        FString MergedText;
        if (Kv.Value.IsValid() && Kv.Value->TryGetString(MergedText))
        {
            if (UEdGraph* G = FindGraphByName(BP, GraphName))
            {
                if (!ReplaceGraphNodes(G, MergedText, Err))
                {
                    OutResponse->SetBoolField(TEXT("ok"), false);
                    OutResponse->SetStringField(TEXT("error"),
                        FString::Printf(TEXT("graph '%s': %s"), *GraphName, *Err));
                    return;
                }
            }
            // Graphs in the request but not on the asset are silently skipped.
        }
    }

    // Best-effort recompile — log on failure but continue.
    FKismetEditorUtilities::CompileBlueprint(BP, EBlueprintCompileOptions::SkipGarbageCollection);

    // Save package.
    FSavePackageArgs SaveArgs;
    SaveArgs.TopLevelFlags = RF_Public | RF_Standalone;
    SaveArgs.SaveFlags = SAVE_NoError;
    SaveArgs.Error = GError;
    const bool bSaved = UPackage::SavePackage(TempPackage, BP, *TempDiskPath, SaveArgs);
    if (!bSaved)
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"), TEXT("SavePackage failed"));
        return;
    }

    OutResponse->SetBoolField(TEXT("ok"), true);
    OutResponse->SetStringField(TEXT("mergedPath"), TempDiskPath.Replace(TEXT("\\"), TEXT("/")));
}
```

- [ ] **Step 3: Commit**

```bash
git add "ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeApplier.h" "ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeApplier.cpp"
git commit -m "feat(ue): MergeApplier — duplicate ancestor, replace nodes, save package"
```

---

### Task 8: UE C++ — register `merge` JSON-RPC handler in the commandlet

**Files:**
- Modify: `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeBinariesExportCommandlet.cpp`

- [ ] **Step 1: Add include + register handler**

In `MergeBinariesExportCommandlet.cpp`, after the existing `#include "AssetExporter.h"` line, add:
```cpp
#include "MergeApplier.h"
```

Then, after the existing `Handlers.Add(TEXT("export"), ...)` block (around line 39), add:

```cpp
    Handlers.Add(TEXT("merge"), [](const TSharedPtr<FJsonObject>& Req, TSharedRef<FJsonObject>& OutResponse)
    {
        FMergeApplier::Apply(Req, OutResponse);
    });
```

- [ ] **Step 2: Commit**

```bash
git add "ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeBinariesExportCommandlet.cpp"
git commit -m "feat(ue): register 'merge' JSON-RPC handler in commandlet"
```

---

### Task 9: Frontend — types, IPC wrappers, CSS

**Files:**
- Modify: `app/src/types.ts`
- Modify: `app/src/ipc.ts`
- Modify: `app/src/styles.css`

- [ ] **Step 1: Add types to types.ts**

Append to `app/src/types.ts`:

```ts
// Rust: graph_diff::ThreeWayNodeStatus — serde(rename_all = "camelCase")
export type ThreeWayNodeStatus =
  | "unchanged"
  | "modifiedInOurs" | "modifiedInTheirs" | "modifiedInBoth"
  | "addedInOurs" | "addedInTheirs" | "addedInBoth" | "addedInBothConflict"
  | "removedInOurs" | "removedInTheirs" | "removedInBoth"
  | "modifyDeleteConflict";

// Rust: graph_diff::ThreeWayGraphDiff — serde(rename_all = "camelCase")
export interface ThreeWayGraphDiff {
  name: string;
  onlyInOurs: boolean;
  onlyInTheirs: boolean;
  onlyInAncestor: boolean;
  nodeStatuses: Record<string, ThreeWayNodeStatus>;
}

export type MergeSide = "ours" | "theirs" | "skip";

export function isConflictStatus(s: ThreeWayNodeStatus): boolean {
  return s === "modifiedInBoth" || s === "addedInBothConflict" || s === "modifyDeleteConflict";
}
```

- [ ] **Step 2: Add IPC wrappers to ipc.ts**

In `app/src/ipc.ts`, update the import line:

```ts
import type { AppMode, AssetSnapshot, PropertyChange, GraphDiff, ThreeWayGraphDiff } from "./types";
```

And append at the bottom:

```ts
export async function diffGraphsThreeWay(
  ancestor: AssetSnapshot,
  ours: AssetSnapshot,
  theirs: AssetSnapshot,
): Promise<ThreeWayGraphDiff[]> {
  return invoke<ThreeWayGraphDiff[]>("diff_graphs_three_way", { ancestor, ours, theirs });
}

export async function applyGraphMerge(
  ancestorPath: string,
  destPath: string,
  mergedGraphs: Record<string, string>,
  options?: { sidecarOverride?: string; hostProjectOverride?: string },
): Promise<void> {
  await invoke<void>("apply_graph_merge", {
    ancestorPath,
    destPath,
    mergedGraphs,
    sidecarOverride: options?.sidecarOverride,
    hostProjectOverride: options?.hostProjectOverride,
  });
}
```

- [ ] **Step 3: Append CSS classes to styles.css**

Append to `app/src/styles.css`:

```css
/* Three-way diff overlay classes (Plan 5). */
.uem-three-way-added {
  outline: 2px solid #2d8a4e !important;
  box-shadow: 0 0 12px rgba(45, 138, 78, 0.5) !important;
}
.uem-three-way-removed {
  outline: 2px solid #8a2d2d !important;
  box-shadow: 0 0 12px rgba(138, 45, 45, 0.5) !important;
}
.uem-three-way-modified {
  outline: 2px solid #8a742d !important;
  box-shadow: 0 0 12px rgba(138, 116, 45, 0.5) !important;
}
.uem-three-way-conflict {
  outline: 2px solid #c038a8 !important;
  box-shadow: 0 0 16px rgba(192, 56, 168, 0.7) !important;
}
.uem-three-way-dimmed {
  opacity: 0.3 !important;
}
```

- [ ] **Step 4: TypeScript check**

From `app/`:
```
pnpm tsc --noEmit
```
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add app/src/types.ts app/src/ipc.ts app/src/styles.css
git commit -m "feat(frontend): three-way types + IPC wrappers + overlay CSS"
```

---

### Task 10: Frontend — `mergeGraphs.ts` (parse + build merged text)

**Files:**
- Create: `app/src/mergeGraphs.ts`

This is pure functions. No vitest is configured in the project; we'll keep the logic small enough to be obviously correct and verify it via the e2e dev test in BlueprintTest.

- [ ] **Step 1: Create `app/src/mergeGraphs.ts`**

```ts
import type { MergeSide, ThreeWayGraphDiff, ThreeWayNodeStatus } from "./types";
import { isConflictStatus } from "./types";

// Mirrors Rust graph_diff::parse_node_blobs: depth-tracking parser for
// nested Begin Object / End Object. Only extracts NodeGuid at depth 1.
export function parseNodeBlobs(text: string): Map<string, string> {
  const result = new Map<string, string>();
  if (!text) return result;
  const lines = text.split(/\r?\n/);
  let inNode = false;
  let depth = 0;
  let buf: string[] = [];
  let guid: string | null = null;

  for (const line of lines) {
    const trimmed = line.trim();
    if (!inNode) {
      if (trimmed.startsWith("Begin Object")) {
        inNode = true;
        depth = 1;
        buf = [line];
        guid = null;
      }
    } else {
      buf.push(line);
      if (trimmed.startsWith("Begin Object")) {
        depth += 1;
      } else if (trimmed.startsWith("End Object")) {
        depth -= 1;
        if (depth === 0) {
          if (guid) result.set(guid, buf.join("\n"));
          inNode = false;
          buf = [];
          guid = null;
        }
      } else if (depth === 1) {
        if (trimmed.startsWith("NodeGuid=")) {
          guid = trimmed.slice("NodeGuid=".length).trim();
        }
      }
    }
  }
  return result;
}

// Default per-GUID merge selection given a status.
//   - non-conflict modifications/additions auto-pick the side that changed
//   - conflicts default to "ours"
//   - unchanged / removedInBoth → no entry needed (no choice)
export function defaultSide(status: ThreeWayNodeStatus): MergeSide | null {
  switch (status) {
    case "unchanged":
    case "removedInBoth":
      return null;
    case "modifiedInOurs":
    case "addedInOurs":
    case "removedInTheirs":  // "ours kept the node" — pick ours
    case "addedInBoth":
      return "ours";
    case "modifiedInTheirs":
    case "addedInTheirs":
    case "removedInOurs":    // "theirs kept the node" — pick theirs
      return "theirs";
    case "modifiedInBoth":
    case "addedInBothConflict":
    case "modifyDeleteConflict":
      return "ours";
  }
}

// Whether a status needs a user choice (conflict).
export function needsChoice(status: ThreeWayNodeStatus): boolean {
  return isConflictStatus(status);
}

// Build the merged text per graph from selections.
//
// For each GUID:
//  - "skip" → drop entirely
//  - "ours" / "theirs" → take that side's blob, falling back to ancestor if absent
// Unchanged GUIDs come from ancestor.
//
// Output order is: unchanged-then-selected, in the order they first appeared in
// (ancestor → ours → theirs) graph text. UE's importer doesn't care about order.
export function buildMergedGraphs(
  threeWayDiffs: ThreeWayGraphDiff[],
  ancestorGraphs: Record<string, string>,
  oursGraphs: Record<string, string>,
  theirsGraphs: Record<string, string>,
  selections: Map<string /* graphName */, Map<string /* guid */, MergeSide>>,
): Record<string, string> {
  const out: Record<string, string> = {};

  for (const diff of threeWayDiffs) {
    const ancBlobs = parseNodeBlobs(ancestorGraphs[diff.name] ?? "");
    const ourBlobs = parseNodeBlobs(oursGraphs[diff.name] ?? "");
    const thrBlobs = parseNodeBlobs(theirsGraphs[diff.name] ?? "");
    const graphSelections = selections.get(diff.name) ?? new Map();

    const chosen: string[] = [];
    const ordered: string[] = Array.from(
      new Set<string>([...ancBlobs.keys(), ...ourBlobs.keys(), ...thrBlobs.keys()]),
    );

    for (const guid of ordered) {
      const status = diff.nodeStatuses[guid];
      if (!status) continue;
      const side = graphSelections.get(guid) ?? defaultSide(status);
      if (side === null) {
        // Unchanged or removed-in-both — emit unchanged from ancestor or skip removed.
        if (status === "unchanged") {
          const blob = ancBlobs.get(guid);
          if (blob) chosen.push(blob);
        }
        continue;
      }
      if (side === "skip") continue;

      let blob: string | undefined;
      if (side === "ours") {
        blob = ourBlobs.get(guid) ?? ancBlobs.get(guid);
      } else {
        blob = thrBlobs.get(guid) ?? ancBlobs.get(guid);
      }
      if (blob) chosen.push(blob);
    }

    out[diff.name] = chosen.join("\n") + (chosen.length ? "\n" : "");
  }

  return out;
}
```

- [ ] **Step 2: TypeScript check**

From `app/`:
```
pnpm tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add app/src/mergeGraphs.ts
git commit -m "feat(frontend): mergeGraphs — parseNodeBlobs, defaultSide, buildMergedGraphs"
```

---

### Task 11: Frontend — `applyThreeWayOverlay` in graphDiff.ts

**Files:**
- Modify: `app/src/graphDiff.ts`

- [ ] **Step 1: Add the new function after the existing `applyDiffOverlay`**

Append to `app/src/graphDiff.ts`:

```ts
import type { MergeSide, ThreeWayGraphDiff, ThreeWayNodeStatus } from "./types";
import { isConflictStatus } from "./types";

export function applyThreeWayOverlay(
  container: HTMLElement,
  diff: ThreeWayGraphDiff,
  side: "ours" | "theirs",
  selections: Map<string, MergeSide>,
): void {
  const nodeEls = container.querySelectorAll("ueb-node");
  nodeEls.forEach((el) => {
    const nodeEl = el as HTMLElement & { entity?: { NodeGuid?: { toString(): string } } };
    const guid = nodeEl.entity?.NodeGuid?.toString();
    if (!guid) return;

    nodeEl.classList.remove(
      "uem-three-way-added",
      "uem-three-way-removed",
      "uem-three-way-modified",
      "uem-three-way-conflict",
      "uem-three-way-dimmed",
    );

    const status: ThreeWayNodeStatus | undefined = diff.nodeStatuses[guid];
    if (!status || status === "unchanged" || status === "removedInBoth") return;

    if (isConflictStatus(status)) {
      nodeEl.classList.add("uem-three-way-conflict");
    } else if (status.startsWith("added")) {
      nodeEl.classList.add("uem-three-way-added");
    } else if (status.startsWith("removed")) {
      nodeEl.classList.add("uem-three-way-removed");
    } else if (status.startsWith("modified")) {
      nodeEl.classList.add("uem-three-way-modified");
    }

    // Dim nodes the user did NOT pick for this side.
    const chosen = selections.get(guid);
    if (chosen === "skip") {
      nodeEl.classList.add("uem-three-way-dimmed");
    } else if (chosen === "ours" && side === "theirs") {
      nodeEl.classList.add("uem-three-way-dimmed");
    } else if (chosen === "theirs" && side === "ours") {
      nodeEl.classList.add("uem-three-way-dimmed");
    }
  });
}
```

- [ ] **Step 2: TypeScript check**

From `app/`:
```
pnpm tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add app/src/graphDiff.ts
git commit -m "feat(frontend): applyThreeWayOverlay — color + dim per side"
```

---

### Task 12: Frontend — GraphPane accepts optional `threeWayDiff` + selection prop

**Files:**
- Modify: `app/src/views/GraphPane.tsx`

- [ ] **Step 1: Update GraphPane to support both 2-way and 3-way overlays**

Replace `app/src/views/GraphPane.tsx` with:

```tsx
import { useEffect, useRef } from "react";
import type { GraphDiff, MergeSide, ThreeWayGraphDiff } from "../types";
import { applyDiffOverlay, applyThreeWayOverlay } from "../graphDiff";
import styles from "./GraphPane.module.css";

interface Props {
  label: string;
  side: "ours" | "theirs";
  graphText: string | undefined;
  diff: GraphDiff | undefined;
  threeWayDiff?: ThreeWayGraphDiff;
  selections?: Map<string, MergeSide>;
}

export default function GraphPane({ label, side, graphText, diff, threeWayDiff, selections }: Props) {
  const canvasRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    canvas.innerHTML = "";
    if (!graphText) return;

    if (!customElements.get("ueb-blueprint")) {
      // eslint-disable-next-line no-console
      console.error("ueb-blueprint custom element not registered");
    }

    const escaped = graphText
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
    canvas.innerHTML =
      `<ueb-blueprint style="display:block;width:100%;height:100%;--ueb-height:100%">` +
      `<template>${escaped}</template>` +
      `</ueb-blueprint>`;

    if (!diff && !threeWayDiff) return;

    let rafId: number | undefined;
    const observer = new MutationObserver(() => {
      if (canvas.querySelector("ueb-node")) {
        observer.disconnect();
        rafId = requestAnimationFrame(() => {
          if (threeWayDiff) {
            applyThreeWayOverlay(canvas, threeWayDiff, side, selections ?? new Map());
          } else if (diff) {
            applyDiffOverlay(canvas, diff, side);
          }
        });
      }
    });
    observer.observe(canvas, { childList: true, subtree: true });

    return () => {
      observer.disconnect();
      if (rafId !== undefined) cancelAnimationFrame(rafId);
      canvas.innerHTML = "";
    };
  }, [graphText, diff, threeWayDiff, selections, side]);

  return (
    <div className={styles.pane}>
      <div className={`${styles.label} ${side === "ours" ? styles.ours : ""}`}>
        {label}
      </div>
      <div ref={canvasRef} className={styles.canvas} />
      {!graphText && <div className={styles.empty}>No graph data</div>}
    </div>
  );
}
```

- [ ] **Step 2: TypeScript check**

```
pnpm tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add app/src/views/GraphPane.tsx
git commit -m "feat(frontend): GraphPane supports three-way overlay + selections prop"
```

---

### Task 13: Frontend — GraphView 3-way mode with conflict picker

**Files:**
- Modify: `app/src/views/GraphView.tsx`
- Modify: `app/src/views/GraphView.module.css`

- [ ] **Step 1: Append picker CSS to GraphView.module.css**

```css
.conflictPicker {
  position: absolute;
  display: flex;
  gap: 2px;
  background: rgba(20, 20, 30, 0.92);
  border: 1px solid #c038a8;
  border-radius: 3px;
  padding: 2px;
  z-index: 100;
  font-size: 9px;
  font-weight: 700;
  pointer-events: auto;
}

.conflictPickerBtn {
  background: transparent;
  border: none;
  color: #ccc;
  padding: 2px 6px;
  cursor: pointer;
  border-radius: 2px;
  font-family: inherit;
  font-size: 9px;
  font-weight: 700;
}

.conflictPickerBtn:hover {
  background: rgba(192, 56, 168, 0.3);
  color: #fff;
}

.conflictPickerBtnActive {
  background: #c038a8;
  color: #fff;
}

.conflictSummary {
  font-size: 10px;
  color: #c038a8;
  font-weight: 700;
  margin-left: 12px;
}

.conflictSummary.noConflicts {
  color: #888;
  font-weight: 400;
}
```

- [ ] **Step 2: Replace GraphView.tsx**

```tsx
import { useEffect, useMemo, useState } from "react";
import type {
  AssetSnapshot,
  GraphDiff,
  MergeSide,
  ThreeWayGraphDiff,
  ThreeWayNodeStatus,
} from "../types";
import { isConflictStatus } from "../types";
import { defaultSide, needsChoice } from "../mergeGraphs";
import GraphPane from "./GraphPane";
import styles from "./GraphView.module.css";

interface Props {
  ours: AssetSnapshot;
  theirs: AssetSnapshot;
  graphDiffs: GraphDiff[];
  /** Optional ancestor — when present, GraphView enters three-way mode. */
  ancestor?: AssetSnapshot;
  threeWayDiffs?: ThreeWayGraphDiff[];
  /** Per-graph per-GUID selection state, owned by Diff.tsx and passed through. */
  selections?: Map<string, Map<string, MergeSide>>;
  onSelectionChange?: (graphName: string, guid: string, side: MergeSide) => void;
}

export default function GraphView({
  ours,
  theirs,
  graphDiffs,
  ancestor,
  threeWayDiffs,
  selections,
  onSelectionChange,
}: Props) {
  const allGraphNames = useMemo(() => {
    const names = new Set<string>([
      ...Object.keys(ours.asset.graphs ?? {}),
      ...Object.keys(theirs.asset.graphs ?? {}),
      ...Object.keys(ancestor?.asset.graphs ?? {}),
    ]);
    const sorted = Array.from(names).sort();
    const eventIdx = sorted.indexOf("EventGraph");
    if (eventIdx > 0) {
      sorted.splice(eventIdx, 1);
      sorted.unshift("EventGraph");
    }
    return sorted;
  }, [ours.asset.graphs, theirs.asset.graphs, ancestor?.asset.graphs]);

  const [activeGraph, setActiveGraph] = useState<string>(
    () => allGraphNames[0] ?? "",
  );
  useEffect(() => {
    if (allGraphNames.length > 0 && !allGraphNames.includes(activeGraph)) {
      setActiveGraph(allGraphNames[0]);
    }
  }, [allGraphNames, activeGraph]);

  const activeDiff = useMemo(
    () => graphDiffs.find((d) => d.name === activeGraph),
    [graphDiffs, activeGraph],
  );
  const activeThreeWayDiff = useMemo(
    () => threeWayDiffs?.find((d) => d.name === activeGraph),
    [threeWayDiffs, activeGraph],
  );
  const activeSelections = useMemo(
    () => selections?.get(activeGraph) ?? new Map<string, MergeSide>(),
    [selections, activeGraph],
  );

  const oursText = ours.asset.graphs?.[activeGraph];
  const theirsText = theirs.asset.graphs?.[activeGraph];

  const onlyInOurs =
    activeThreeWayDiff?.onlyInOurs ??
    activeDiff?.onlyInOurs ??
    (oursText != null && theirsText == null);
  const onlyInTheirs =
    activeThreeWayDiff?.onlyInTheirs ??
    activeDiff?.onlyInTheirs ??
    (oursText == null && theirsText != null);

  // Conflict summary for the toolbar (only meaningful in 3-way mode).
  const conflictGuids = useMemo(() => {
    if (!activeThreeWayDiff) return [] as string[];
    return Object.entries(activeThreeWayDiff.nodeStatuses)
      .filter(([, s]) => isConflictStatus(s as ThreeWayNodeStatus))
      .map(([guid]) => guid);
  }, [activeThreeWayDiff]);

  return (
    <div className={styles.container}>
      <div className={styles.toolbar}>
        <span>Graph:</span>
        <select
          className={styles.graphSelect}
          value={activeGraph}
          onChange={(e) => setActiveGraph(e.target.value)}
        >
          {allGraphNames.map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
        {onlyInOurs && (
          <span className={`${styles.badge} ${styles.badgeOurs}`}>only in Ours</span>
        )}
        {onlyInTheirs && (
          <span className={`${styles.badge} ${styles.badgeTheirs}`}>only in Theirs</span>
        )}
        {activeThreeWayDiff && (
          <span className={`${styles.conflictSummary} ${conflictGuids.length === 0 ? styles.noConflicts : ""}`}>
            {conflictGuids.length === 0
              ? "no conflicts"
              : `${conflictGuids.length} conflict${conflictGuids.length === 1 ? "" : "s"}`}
          </span>
        )}
      </div>

      <div className={styles.split} style={{ position: "relative" }}>
        <GraphPane
          label="Ours"
          side="ours"
          graphText={oursText}
          diff={activeThreeWayDiff ? undefined : activeDiff}
          threeWayDiff={activeThreeWayDiff}
          selections={activeSelections}
        />
        <GraphPane
          label="Theirs"
          side="theirs"
          graphText={theirsText}
          diff={activeThreeWayDiff ? undefined : activeDiff}
          threeWayDiff={activeThreeWayDiff}
          selections={activeSelections}
        />
        {activeThreeWayDiff && onSelectionChange && (
          <ConflictPickers
            diff={activeThreeWayDiff}
            selections={activeSelections}
            onPick={(guid, side) => onSelectionChange(activeGraph, guid, side)}
          />
        )}
      </div>
    </div>
  );
}

interface PickersProps {
  diff: ThreeWayGraphDiff;
  selections: Map<string, MergeSide>;
  onPick: (guid: string, side: MergeSide) => void;
}

// Renders a small floating picker per conflicting node. Position-tracking
// uses a MutationObserver to find <ueb-node> elements and align to them.
function ConflictPickers({ diff, selections, onPick }: PickersProps) {
  const [positions, setPositions] = useState<Array<{ guid: string; top: number; left: number }>>([]);

  useEffect(() => {
    const conflicts = Object.entries(diff.nodeStatuses)
      .filter(([, s]) => isConflictStatus(s as ThreeWayNodeStatus))
      .map(([guid]) => guid);

    if (conflicts.length === 0) {
      setPositions([]);
      return;
    }

    // The pickers position over the OURS pane (first pane). Find its DOM.
    const container = document.querySelector(`.${styles.split}`);
    if (!container) return;
    const oursPane = container.children[0] as HTMLElement | undefined;
    if (!oursPane) return;

    function recompute() {
      const next: Array<{ guid: string; top: number; left: number }> = [];
      const containerRect = oursPane!.getBoundingClientRect();
      const nodeEls = oursPane!.querySelectorAll("ueb-node");
      nodeEls.forEach((el) => {
        const nodeEl = el as HTMLElement & { entity?: { NodeGuid?: { toString(): string } } };
        const guid = nodeEl.entity?.NodeGuid?.toString();
        if (!guid || !conflicts.includes(guid)) return;
        const r = el.getBoundingClientRect();
        next.push({
          guid,
          top: r.top - containerRect.top,
          left: r.left - containerRect.left + r.width / 2 - 40,
        });
      });
      setPositions(next);
    }

    recompute();
    const observer = new MutationObserver(recompute);
    observer.observe(oursPane, { childList: true, subtree: true, attributes: true });
    const interval = window.setInterval(recompute, 1000); // catch scroll/zoom changes

    return () => {
      observer.disconnect();
      window.clearInterval(interval);
    };
  }, [diff]);

  return (
    <>
      {positions.map(({ guid, top, left }) => {
        const status = diff.nodeStatuses[guid];
        const cur = selections.get(guid) ?? defaultSide(status);
        return (
          <div
            key={guid}
            className={styles.conflictPicker}
            style={{ top, left }}
          >
            <button
              className={`${styles.conflictPickerBtn} ${cur === "ours" ? styles.conflictPickerBtnActive : ""}`}
              onClick={() => onPick(guid, "ours")}
              title="Take Ours"
            >
              O
            </button>
            <button
              className={`${styles.conflictPickerBtn} ${cur === "theirs" ? styles.conflictPickerBtnActive : ""}`}
              onClick={() => onPick(guid, "theirs")}
              title="Take Theirs"
            >
              T
            </button>
            <button
              className={`${styles.conflictPickerBtn} ${cur === "skip" ? styles.conflictPickerBtnActive : ""}`}
              onClick={() => onPick(guid, "skip")}
              title="Skip (omit this node from the merge)"
            >
              —
            </button>
          </div>
        );
      })}
    </>
  );
}
```

Note: `needsChoice` is imported but unused — TypeScript will warn. Remove from the import line if so. Final import is:
```ts
import { defaultSide } from "../mergeGraphs";
```

- [ ] **Step 3: TypeScript check**

```
pnpm tsc --noEmit
```
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add app/src/views/GraphView.tsx app/src/views/GraphView.module.css
git commit -m "feat(frontend): GraphView three-way mode with conflict picker overlay"
```

---

### Task 14: Frontend — Resolve gains optional "Take Both" button

**Files:**
- Modify: `app/src/views/Resolve.tsx`
- Modify: `app/src/views/Resolve.module.css`

- [ ] **Step 1: Update Resolve.tsx**

Replace `app/src/views/Resolve.tsx` with:

```tsx
import styles from "./Resolve.module.css";

interface Props {
  onTakeOurs: () => void;
  onTakeTheirs: () => void;
  onTakeBoth?: () => void;
  onAbort: () => void;
  disabled: boolean;
  bothLabel?: string;
}

export default function Resolve({ onTakeOurs, onTakeTheirs, onTakeBoth, onAbort, disabled, bothLabel }: Props) {
  return (
    <footer className={styles.bar}>
      <button className={styles.btn} onClick={onTakeOurs} disabled={disabled}>
        Take Ours
      </button>
      <button className={styles.btn} onClick={onTakeTheirs} disabled={disabled}>
        Take Theirs
      </button>
      {onTakeBoth && (
        <button
          className={`${styles.btn} ${styles.both}`}
          onClick={onTakeBoth}
          disabled={disabled}
        >
          {bothLabel ?? "Take Both"}
        </button>
      )}
      <span className={styles.spacer} />
      <button
        className={`${styles.btn} ${styles.abort}`}
        onClick={onAbort}
        disabled={disabled}
      >
        Abort
      </button>
    </footer>
  );
}
```

- [ ] **Step 2: Add `.both` style to Resolve.module.css**

Append to `app/src/views/Resolve.module.css`:

```css
.both {
  background: rgba(0, 137, 255, 0.15);
  border-color: rgba(0, 137, 255, 0.5);
  color: #79bdff;
}

.both:hover:not(:disabled) {
  background: rgba(0, 137, 255, 0.28);
  border-color: var(--ue-accent);
  color: #fff;
}
```

- [ ] **Step 3: TypeScript check**

```
pnpm tsc --noEmit
```
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add app/src/views/Resolve.tsx app/src/views/Resolve.module.css
git commit -m "feat(frontend): Resolve — optional Take Both button"
```

---

### Task 15: Frontend — Diff.tsx wires ancestor + Take Both

**Files:**
- Modify: `app/src/views/Diff.tsx`

- [ ] **Step 1: Replace Diff.tsx**

```tsx
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  applyGraphMerge,
  applyResolution,
  closeWithExit,
  diffGraphs,
  diffGraphsThreeWay,
  diffSnapshots,
  exportAsset,
} from "../ipc";
import type {
  AssetSnapshot,
  GraphDiff,
  MergeSide,
  PropertyChange,
  ThreeWayGraphDiff,
  ThreeWayNodeStatus,
} from "../types";
import { isConflictStatus } from "../types";
import { buildMergedGraphs, defaultSide } from "../mergeGraphs";
import GraphView from "./GraphView";
import PropertiesDiff from "./PropertiesDiff";
import Resolve from "./Resolve";
import styles from "./Diff.module.css";

interface Props {
  oursPath: string;
  theirsPath: string;
  destPath: string;
  /** Git's %O (merge base). When provided + asset is Blueprint, enables Take Both. */
  ancestorPath?: string;
}

type Status =
  | { kind: "loading" }
  | { kind: "error"; message: string }
  | {
      kind: "ready";
      ours: AssetSnapshot;
      theirs: AssetSnapshot;
      ancestor?: AssetSnapshot;
      changes: PropertyChange[];
      graphDiffs: GraphDiff[];
      threeWayDiffs?: ThreeWayGraphDiff[];
    };

type Tab = "graph" | "properties";

export default function Diff({ oursPath, theirsPath, destPath, ancestorPath }: Props) {
  const [status, setStatus] = useState<Status>({ kind: "loading" });
  const [resolving, setResolving] = useState(false);
  const [activeTab, setActiveTab] = useState<Tab>("graph");
  // Per-graph per-GUID selections. Initialised from defaults once threeWayDiffs arrive.
  const [selections, setSelections] = useState<Map<string, Map<string, MergeSide>>>(new Map());

  useEffect(() => {
    setActiveTab("graph");
  }, [oursPath, theirsPath]);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const [ours, theirs, ancestor] = await Promise.all([
          exportAsset(oursPath),
          exportAsset(theirsPath),
          ancestorPath ? exportAsset(ancestorPath) : Promise.resolve(undefined),
        ]);
        const [changes, graphDiffs] = await Promise.all([
          diffSnapshots(ours, theirs),
          diffGraphs(ours, theirs),
        ]);
        let threeWayDiffs: ThreeWayGraphDiff[] | undefined;
        if (ancestor && ours.asset.class === "Blueprint") {
          threeWayDiffs = await diffGraphsThreeWay(ancestor, ours, theirs);
        }
        if (!cancelled) {
          setStatus({ kind: "ready", ours, theirs, ancestor, changes, graphDiffs, threeWayDiffs });
          if (threeWayDiffs) {
            // Seed selections from defaults so we always have a valid choice.
            const seed = new Map<string, Map<string, MergeSide>>();
            for (const d of threeWayDiffs) {
              const m = new Map<string, MergeSide>();
              for (const [guid, st] of Object.entries(d.nodeStatuses)) {
                const def = defaultSide(st as ThreeWayNodeStatus);
                if (def !== null) m.set(guid, def);
              }
              seed.set(d.name, m);
            }
            setSelections(seed);
          }
        }
      } catch (e) {
        if (!cancelled) setStatus({ kind: "error", message: String(e) });
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, [oursPath, theirsPath, ancestorPath]);

  const changedPaths = useMemo(() => {
    if (status.kind !== "ready") return new Set<string>();
    const s = new Set<string>();
    for (const c of status.changes) s.add(c.path);
    return s;
  }, [status]);

  const onSelectionChange = useCallback((graphName: string, guid: string, side: MergeSide) => {
    setSelections((prev) => {
      const next = new Map(prev);
      const inner = new Map(next.get(graphName) ?? new Map<string, MergeSide>());
      inner.set(guid, side);
      next.set(graphName, inner);
      return next;
    });
  }, []);

  const conflictCount = useMemo(() => {
    if (status.kind !== "ready" || !status.threeWayDiffs) return 0;
    let n = 0;
    for (const d of status.threeWayDiffs) {
      for (const st of Object.values(d.nodeStatuses)) {
        if (isConflictStatus(st as ThreeWayNodeStatus)) n += 1;
      }
    }
    return n;
  }, [status]);

  async function resolve(kind: "ours" | "theirs" | "abort" | "both") {
    setResolving(true);
    try {
      if (kind === "abort") {
        await closeWithExit(1);
        return;
      }
      if (kind === "both") {
        if (status.kind !== "ready" || !status.threeWayDiffs || !status.ancestor || !ancestorPath) {
          throw new Error("Take Both is not available — missing ancestor or three-way diff");
        }
        const merged = buildMergedGraphs(
          status.threeWayDiffs,
          status.ancestor.asset.graphs ?? {},
          status.ours.asset.graphs ?? {},
          status.theirs.asset.graphs ?? {},
          selections,
        );
        await applyGraphMerge(ancestorPath, destPath, merged);
        await closeWithExit(0);
        return;
      }
      await applyResolution(kind, oursPath, theirsPath, destPath);
      await closeWithExit(0);
    } catch (e) {
      setStatus({ kind: "error", message: String(e) });
      setResolving(false);
    }
  }

  if (status.kind === "loading") {
    return <div className={styles.loading}>Loading conflict…</div>;
  }
  if (status.kind === "error") {
    return (
      <div className={styles.error}>
        <p>Failed to load:</p>
        <pre>{status.message}</pre>
        <Resolve
          onTakeOurs={() => resolve("ours")}
          onTakeTheirs={() => resolve("theirs")}
          onAbort={() => resolve("abort")}
          disabled={resolving}
        />
      </div>
    );
  }

  const isBlueprint =
    status.ours.asset.class === "Blueprint" ||
    status.theirs.asset.class === "Blueprint";

  const showTakeBoth = isBlueprint && status.threeWayDiffs != null;
  const bothLabel =
    conflictCount > 0
      ? `Take Both (resolve ${conflictCount} conflict${conflictCount === 1 ? "" : "s"})`
      : "Take Both";

  return (
    <div className={styles.container}>
      <header className={styles.header}>
        <span>Conflict: {destPath}</span>
        <span className={styles.summary}>
          {status.changes.length} property change{status.changes.length === 1 ? "" : "s"}
          {" · "}
          ours sha {status.ours.package.savedHash.slice(0, 14)}…
          {" · "}
          theirs sha {status.theirs.package.savedHash.slice(0, 14)}…
        </span>
      </header>

      {isBlueprint && (
        <div className={styles.tabRow}>
          <button
            className={`${styles.tab} ${activeTab === "graph" ? styles.tabActive : ""}`}
            onClick={() => setActiveTab("graph")}
          >
            Graph
          </button>
          <button
            className={`${styles.tab} ${activeTab === "properties" ? styles.tabActive : ""}`}
            onClick={() => setActiveTab("properties")}
          >
            Properties
          </button>
        </div>
      )}

      {(!isBlueprint || activeTab === "properties") && (
        <div className={styles.panes}>
          <PropertiesDiff
            title="Ours"
            properties={status.ours.asset.properties}
            highlight={changedPaths}
          />
          <PropertiesDiff
            title="Theirs"
            properties={status.theirs.asset.properties}
            highlight={changedPaths}
          />
        </div>
      )}

      {isBlueprint && activeTab === "graph" && (
        <GraphView
          ours={status.ours}
          theirs={status.theirs}
          graphDiffs={status.graphDiffs}
          ancestor={status.ancestor}
          threeWayDiffs={status.threeWayDiffs}
          selections={selections}
          onSelectionChange={onSelectionChange}
        />
      )}

      <Resolve
        onTakeOurs={() => resolve("ours")}
        onTakeTheirs={() => resolve("theirs")}
        onTakeBoth={showTakeBoth ? () => resolve("both") : undefined}
        onAbort={() => resolve("abort")}
        disabled={resolving}
        bothLabel={bothLabel}
      />
    </div>
  );
}
```

- [ ] **Step 2: TypeScript check**

```
pnpm tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add app/src/views/Diff.tsx
git commit -m "feat(frontend): Diff wires ancestor + selections + Take Both"
```

---

### Task 16: Frontend — App.tsx passes `ancestor` to Diff

**Files:**
- Modify: `app/src/App.tsx`

- [ ] **Step 1: Pass ancestorPath**

In `app/src/App.tsx`, replace the `gitDriverGui` return statement:

```tsx
  if (mode.kind === "gitDriverGui") {
    return (
      <Diff
        oursPath={mode.ours}
        theirsPath={mode.theirs}
        destPath={mode.ours}
        ancestorPath={mode.ancestor}
      />
    );
  }
```

- [ ] **Step 2: TypeScript check**

```
pnpm tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add app/src/App.tsx
git commit -m "feat(frontend): pass ancestorPath from gitDriverGui to Diff"
```

---

### Task 17: Frontend — BlueprintTest renders 3-way

**Files:**
- Modify: `app/src/views/BlueprintTest.tsx`

This makes `pnpm dev` render the 3-way view, exercising the picker without needing the Tauri/IPC layer.

- [ ] **Step 1: Add ancestor fixture and 3-way diffs**

Replace the bottom portion of `app/src/views/BlueprintTest.tsx` starting from the `const EVENT_GRAPH_THEIRS = ...;` declaration's `END` (line ~147) onward. Add a new constant `EVENT_GRAPH_ANCESTOR`, build a third snapshot, and supply `threeWayDiffs` to `GraphView`.

Insert this block AFTER the existing `EVENT_GRAPH_THEIRS` template literal:

```ts
const EVENT_GRAPH_ANCESTOR = `\
Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name="K2Node_Event_BeginPlay"
   EventReference=(MemberParent=Class'"/Script/Engine.Actor"',MemberName="ReceiveBeginPlay")
   bOverrideFunction=True
   NodeGuid=${G_BEGINPLAY}
   NodePosX=-80
   NodePosY=0
   CustomProperties Pin (PinId=C0000000000000000000000000000010,PinName="OutputDelegate",Direction="EGPD_Output",PinType.PinCategory="delegate",PinType.PinSubCategory="MulticastDelegateProperty",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
   CustomProperties Pin (PinId=C0000000000000000000000000000011,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health C0000000000000000000000000000020,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableSet Name="K2Node_VariableSet_Health"
   VariableReference=(MemberName="Health",MemberGuid=AABBCC00000000000000000000000001)
   NodeGuid=${G_SET_HEALTH}
   NodePosX=180
   NodePosY=0
   CustomProperties Pin (PinId=C0000000000000000000000000000020,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Event_BeginPlay C0000000000000000000000000000011,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000021,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 C0000000000000000000000000000030,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000022,PinName="Health",Direction="EGPD_Input",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,DefaultValue="0.0",)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_IfThenElse Name="K2Node_IfThenElse_0"
   NodeGuid=${G_BRANCH}
   NodePosX=460
   NodePosY=0
   CustomProperties Pin (PinId=C0000000000000000000000000000030,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health C0000000000000000000000000000021,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000031,PinName="Condition",Direction="EGPD_Input",PinType.PinCategory="bool",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 C0000000000000000000000000000061,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000032,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_CallFunction_PrintTrue C0000000000000000000000000000040,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000033,PinName="else",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name="K2Node_CallFunction_PrintTrue"
   FunctionReference=(MemberParent=Class'"/Script/Engine.KismetSystemLibrary"',MemberName="PrintString")
   NodeGuid=${G_PRINT_TRUE}
   NodePosX=760
   NodePosY=-100
   CustomProperties Pin (PinId=C0000000000000000000000000000040,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 C0000000000000000000000000000032,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000041,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
   CustomProperties Pin (PinId=C0000000000000000000000000000042,PinName="InString",Direction="EGPD_Input",PinType.PinCategory="string",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 C0000000000000000000000000000061,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableGet Name="K2Node_VariableGet_Health"
   VariableReference=(MemberName="Health",MemberGuid=AABBCC00000000000000000000000001)
   NodeGuid=${G_GET_HEALTH}
   NodePosX=380
   NodePosY=220
   CustomProperties Pin (PinId=C0000000000000000000000000000050,PinName="Health",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 C0000000000000000000000000000060,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_Knot Name="K2Node_Knot_0"
   NodeGuid=${G_KNOT}
   NodePosX=560
   NodePosY=180
   CustomProperties Pin (PinId=C0000000000000000000000000000060,PinName="InputPin",Direction="EGPD_Input",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableGet_Health C0000000000000000000000000000050,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000061,PinName="OutputPin",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 C0000000000000000000000000000031,K2Node_CallFunction_PrintTrue C0000000000000000000000000000042,),)
End Object
`;
```

Then change the import at the top:

```ts
import type { AssetSnapshot, ThreeWayGraphDiff } from "../types";
```

And replace the existing `const DIFFS: GraphDiff[]` block + default-export with:

```ts
const ANCESTOR = makeSnapshot({ EventGraph: EVENT_GRAPH_ANCESTOR });

// Three-way: SET_HEALTH = ModifiedInTheirs (theirs added MaxHealth pin)
//            PRINT_FALSE = AddedInOurs
//            GET_MAX = AddedInTheirs
const THREE_WAY_DIFFS: ThreeWayGraphDiff[] = [
  {
    name: "EventGraph",
    onlyInOurs: false,
    onlyInTheirs: false,
    onlyInAncestor: false,
    nodeStatuses: {
      [G_BEGINPLAY]:   "unchanged",
      [G_SET_HEALTH]:  "modifiedInTheirs",
      [G_BRANCH]:      "unchanged",
      [G_PRINT_TRUE]:  "unchanged",
      [G_GET_HEALTH]:  "unchanged",
      [G_KNOT]:        "unchanged",
      [G_PRINT_FALSE]: "addedInOurs",
      [G_GET_MAX]:     "addedInTheirs",
    },
  },
];

export default function BlueprintTest() {
  return (
    <div
      style={{
        height: "100vh",
        display: "flex",
        flexDirection: "column",
        background: "var(--ue-bg-deep)",
      }}
    >
      <div
        style={{
          padding: "8px 14px",
          background: "linear-gradient(to bottom, #1f1f1f, #161616)",
          borderBottom: "1px solid var(--ue-border)",
          fontSize: 11,
          color: "var(--ue-text-dim)",
          letterSpacing: "0.04em",
        }}
      >
        BP_Base 3-way conflict — Alice (Ours) adds False-branch PrintString; Bob (Theirs) feeds SET Health from MaxHealth. No real conflict — Take Both auto-merges.
      </div>
      <GraphView
        ours={OURS}
        theirs={THEIRS}
        graphDiffs={[]}
        ancestor={ANCESTOR}
        threeWayDiffs={THREE_WAY_DIFFS}
        selections={new Map()}
        onSelectionChange={() => {}}
      />
    </div>
  );
}
```

Also remove the now-unused `GraphDiff` import (TypeScript will warn).

- [ ] **Step 2: TypeScript check + browser smoke**

```
pnpm tsc --noEmit
```
Expected: no errors.

```
pnpm dev
```
Expected: in browser at `http://127.0.0.1:1420`, both panes render the BP_Base graph with:
- `K2Node_VariableSet_Health` outlined amber on the theirs pane (modifiedInTheirs)
- `K2Node_CallFunction_PrintFalse` outlined green on the ours pane (addedInOurs)
- `K2Node_VariableGet_MaxHealth` outlined green on the theirs pane (addedInTheirs)
- Toolbar shows "no conflicts"

Manually verify, then kill the dev server.

- [ ] **Step 3: Commit**

```bash
git add app/src/views/BlueprintTest.tsx
git commit -m "feat(frontend): BlueprintTest renders three-way GraphView fixture"
```

---

### Task 18: Final verification

- [ ] **Step 1: Rust full test suite**

From `app/src-tauri/`:
```
cargo test
```
Expected: all ~65 tests pass.

- [ ] **Step 2: TypeScript build**

From `app/`:
```
pnpm tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Frontend smoke (browser-only)**

From `app/`:
```
pnpm dev
```
Visit `http://127.0.0.1:1420` — BlueprintTest renders the 3-way view (per Task 17 expectations). Kill the dev server.

- [ ] **Step 4: Final commit (only if anything uncommitted)**

```bash
git status
```
If anything is unstaged, investigate and commit before declaring complete.
