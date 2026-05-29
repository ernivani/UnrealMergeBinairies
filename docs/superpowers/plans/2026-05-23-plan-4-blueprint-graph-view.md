# Blueprint Graph View Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Blueprint graph view tab to the Tauri diff window that renders UE Blueprint nodes/pins/wires side-by-side (Ours vs Theirs), with changed/added/removed nodes highlighted by border color.

**Architecture:** C++ commandlet exports raw UE serialization text per graph via `FBlueprintEditorUtils::ExportNodesToText`; Rust parses node GUIDs and computes per-node diff status; the frontend uses the `ueblueprint` npm package (web component) to render pixel-identical Blueprint graphs, then injects CSS diff borders by querying `ueb-node` elements and accessing `el.entity.NodeGuid.toString()`.

**Tech Stack:** UE 5.6 C++ (`FBlueprintEditorUtils`, `UEdGraph`), Rust (serde, string parsing - no regex crate), React 18 + TypeScript, `ueblueprint` npm web component library, Tauri 2 IPC, CSS Modules.

---

## File Map

| File | Change |
|---|---|
| `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/BlueprintExporter.h` | **New** |
| `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/BlueprintExporter.cpp` | **New** |
| `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/AssetExporter.cpp` | Add `#include` + graph export call after line 147 |
| `app/src-tauri/src/graph_diff.rs` | **New** - NodeStatus, GraphDiff, parse + diff logic, unit tests |
| `app/src-tauri/src/schema.rs` | Add `graphs: Option<HashMap<String, String>>` to `Asset` |
| `app/src-tauri/src/ipc.rs` | Add `diff_graphs` command + `diff_graphs_inner` |
| `app/src-tauri/src/lib.rs` | Declare `graph_diff` module; re-export `GraphDiff` |
| `app/src-tauri/src/main.rs` | Register `diff_graphs` in `generate_handler!` |
| `app/package.json` | Add `ueblueprint` dependency |
| `app/src/main.tsx` | Import `ueblueprint/dist/css/ueb-style.min.css` |
| `app/src/types.ts` | Add `NodeStatus`, `GraphDiff`; add `graphs` to `Asset` |
| `app/src/ipc.ts` | Add `diffGraphs()` |
| `app/src/graphDiff.ts` | **New** - `applyDiffOverlay` DOM function |
| `app/src/views/GraphPane.tsx` | **New** |
| `app/src/views/GraphPane.module.css` | **New** |
| `app/src/views/GraphView.tsx` | **New** |
| `app/src/views/GraphView.module.css` | **New** |
| `app/src/views/Diff.tsx` | Add Graph/Properties tab switcher + GraphView |
| `app/src/styles.css` | Add `.uem-diff-*` border classes |

---

### Task 1: C++ BlueprintExporter - new header and implementation

**Files:**
- Create: `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/BlueprintExporter.h`
- Create: `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/BlueprintExporter.cpp`

There is no unit test for this task - it requires a live UE editor context. The golden test in Task 2 serves as the integration test.

- [ ] **Step 1: Create BlueprintExporter.h**

```cpp
#pragma once

#include "CoreMinimal.h"
#include "Dom/JsonObject.h"

class UBlueprint;

struct FGraphExport
{
    FString GraphName;
    FString GraphText;
};

class FBlueprintExporter
{
public:
    // Returns one FGraphExport per graph (EventGraph, function graphs, macro graphs).
    // Returns empty array if Blueprint is null or has no graphs.
    static TArray<FGraphExport> ExportGraphs(UBlueprint* Blueprint);
};
```

- [ ] **Step 2: Create BlueprintExporter.cpp**

```cpp
#include "BlueprintExporter.h"

#include "Engine/Blueprint.h"
#include "EdGraph/EdGraph.h"
#include "Kismet2/BlueprintEditorUtils.h"

TArray<FGraphExport> FBlueprintExporter::ExportGraphs(UBlueprint* Blueprint)
{
    TArray<FGraphExport> Result;
    if (!Blueprint) { return Result; }

    TArray<UEdGraph*> AllGraphs;
    AllGraphs.Append(Blueprint->UbergraphPages);
    AllGraphs.Append(Blueprint->FunctionGraphs);
    AllGraphs.Append(Blueprint->MacroGraphs);

    for (UEdGraph* Graph : AllGraphs)
    {
        if (!Graph) { continue; }

        TSet<UEdGraphNode*> NodeSet(Graph->Nodes);
        FString ExportedText;
        FBlueprintEditorUtils::ExportNodesToText(NodeSet, ExportedText);

        FGraphExport Export;
        Export.GraphName = Graph->GetName();
        Export.GraphText = MoveTemp(ExportedText);
        Result.Add(MoveTemp(Export));
    }
    return Result;
}
```

- [ ] **Step 3: Commit**

```bash
git add "ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/BlueprintExporter.h"
git add "ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/BlueprintExporter.cpp"
git commit -m "feat(ue): add BlueprintExporter - ExportNodesToText per graph"
```

---

### Task 2: AssetExporter.cpp - call BlueprintExporter, add graphs to JSON

**Files:**
- Modify: `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/AssetExporter.cpp`

Note: `UnrealEd` is already present in `MergeBinariesExport.Build.cs` - no build file change needed.

The insertion point is **after line 147** (`Asset->SetArrayField(TEXT("properties"), Entries);`) and **before** line 149 (`OutResponse->SetBoolField(TEXT("ok"), true);`).

- [ ] **Step 1: Add include at top of AssetExporter.cpp**

After the existing includes (around line 16), add:

```cpp
#include "BlueprintExporter.h"
#include "Engine/Blueprint.h"
```

- [ ] **Step 2: Add graph export block after the properties array**

After line 147 (`Asset->SetArrayField(TEXT("properties"), Entries);`), insert:

```cpp
    // Blueprint graph export - only for Blueprint assets.
    if (UBlueprint* BP = Cast<UBlueprint>(Primary))
    {
        const TSharedRef<FJsonObject> GraphsObj = MakeShared<FJsonObject>();
        for (const FGraphExport& GE : FBlueprintExporter::ExportGraphs(BP))
        {
            GraphsObj->SetStringField(GE.GraphName, GE.GraphText);
        }
        Asset->SetObjectField(TEXT("graphs"), GraphsObj);
    }
    else
    {
        Asset->SetField(TEXT("graphs"), MakeShared<FJsonValueNull>());
    }
```

- [ ] **Step 3: Commit**

```bash
git add "ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/AssetExporter.cpp"
git commit -m "feat(ue): export Blueprint graphs as UE serialization text in asset JSON"
```

---

### Task 3: graph_diff.rs - GUID parsing and diff logic with unit tests

**Files:**
- Create: `app/src-tauri/src/graph_diff.rs`

This is pure Rust with no UE dependency - full unit test coverage.

**Important:** The Cargo.toml has no `regex` crate. Use string parsing only.

- [ ] **Step 1: Write the failing tests first**

Create `app/src-tauri/src/graph_diff.rs` with the test module only:

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeStatus {
    Added,
    Removed,
    Changed,
    Unchanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDiff {
    pub name: String,
    pub only_in_ours: bool,
    pub only_in_theirs: bool,
    pub node_statuses: HashMap<String, NodeStatus>,
}

fn extract_guid(node_text: &str) -> Option<String> {
    for line in node_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("NodeGuid=") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn parse_node_blobs(text: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for part in text.split("Begin Object").skip(1) {
        let end_idx = part.find("End Object").unwrap_or(part.len());
        let node_text = &part[..end_idx];
        if let Some(guid) = extract_guid(node_text) {
            result.insert(guid, node_text.to_string());
        }
    }
    result
}

pub fn diff_graphs_inner(
    ours_graphs: &HashMap<String, String>,
    theirs_graphs: &HashMap<String, String>,
) -> Vec<GraphDiff> {
    let mut all_names: std::collections::BTreeSet<String> = Default::default();
    all_names.extend(ours_graphs.keys().cloned());
    all_names.extend(theirs_graphs.keys().cloned());

    let mut result = Vec::new();
    for name in all_names {
        let ours_text = ours_graphs.get(&name);
        let theirs_text = theirs_graphs.get(&name);

        let only_in_ours = ours_text.is_some() && theirs_text.is_none();
        let only_in_theirs = ours_text.is_none() && theirs_text.is_some();

        let ours_nodes = ours_text.map(|t| parse_node_blobs(t)).unwrap_or_default();
        let theirs_nodes = theirs_text.map(|t| parse_node_blobs(t)).unwrap_or_default();

        let mut node_statuses = HashMap::new();

        for (guid, ours_blob) in &ours_nodes {
            if let Some(theirs_blob) = theirs_nodes.get(guid) {
                if ours_blob == theirs_blob {
                    node_statuses.insert(guid.clone(), NodeStatus::Unchanged);
                } else {
                    node_statuses.insert(guid.clone(), NodeStatus::Changed);
                }
            } else {
                node_statuses.insert(guid.clone(), NodeStatus::Removed);
            }
        }

        for guid in theirs_nodes.keys() {
            if !ours_nodes.contains_key(guid) {
                node_statuses.insert(guid.clone(), NodeStatus::Added);
            }
        }

        result.push(GraphDiff {
            name,
            only_in_ours,
            only_in_theirs,
            node_statuses,
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_graphs(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    const NODE_A: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name=\"K2Node_Event_0\"
   NodeGuid=AAAAAAAA000000000000000000000001
   NodePosX=100
End Object
";

    const NODE_A_CHANGED: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name=\"K2Node_Event_0\"
   NodeGuid=AAAAAAAA000000000000000000000001
   NodePosX=200
End Object
";

    const NODE_B: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_0\"
   NodeGuid=BBBBBBBB000000000000000000000002
   NodePosX=300
End Object
";

    #[test]
    fn test_diff_unchanged() {
        let ours = make_graphs(&[("EventGraph", NODE_A)]);
        let theirs = make_graphs(&[("EventGraph", NODE_A)]);
        let diffs = diff_graphs_inner(&ours, &theirs);
        assert_eq!(diffs.len(), 1);
        let diff = &diffs[0];
        assert_eq!(diff.name, "EventGraph");
        assert!(!diff.only_in_ours);
        assert!(!diff.only_in_theirs);
        assert_eq!(
            diff.node_statuses.get("AAAAAAAA000000000000000000000001"),
            Some(&NodeStatus::Unchanged)
        );
    }

    #[test]
    fn test_diff_changed() {
        let ours = make_graphs(&[("EventGraph", NODE_A)]);
        let theirs = make_graphs(&[("EventGraph", NODE_A_CHANGED)]);
        let diffs = diff_graphs_inner(&ours, &theirs);
        assert_eq!(
            diffs[0].node_statuses.get("AAAAAAAA000000000000000000000001"),
            Some(&NodeStatus::Changed)
        );
    }

    #[test]
    fn test_diff_removed() {
        let ours = make_graphs(&[("EventGraph", NODE_A)]);
        let theirs = make_graphs(&[("EventGraph", "")]);
        let diffs = diff_graphs_inner(&ours, &theirs);
        assert_eq!(
            diffs[0].node_statuses.get("AAAAAAAA000000000000000000000001"),
            Some(&NodeStatus::Removed)
        );
    }

    #[test]
    fn test_diff_added() {
        let ours = make_graphs(&[("EventGraph", "")]);
        let theirs = make_graphs(&[("EventGraph", NODE_B)]);
        let diffs = diff_graphs_inner(&ours, &theirs);
        assert_eq!(
            diffs[0].node_statuses.get("BBBBBBBB000000000000000000000002"),
            Some(&NodeStatus::Added)
        );
    }

    #[test]
    fn test_graph_only_in_ours() {
        let ours = make_graphs(&[("EventGraph", NODE_A), ("MyFunction", NODE_B)]);
        let theirs = make_graphs(&[("EventGraph", NODE_A)]);
        let diffs = diff_graphs_inner(&ours, &theirs);
        let my_fn = diffs.iter().find(|d| d.name == "MyFunction").unwrap();
        assert!(my_fn.only_in_ours);
        assert!(!my_fn.only_in_theirs);
    }

    #[test]
    fn test_graph_only_in_theirs() {
        let ours = make_graphs(&[("EventGraph", NODE_A)]);
        let theirs = make_graphs(&[("EventGraph", NODE_A), ("NewGraph", NODE_B)]);
        let diffs = diff_graphs_inner(&ours, &theirs);
        let new_graph = diffs.iter().find(|d| d.name == "NewGraph").unwrap();
        assert!(!new_graph.only_in_ours);
        assert!(new_graph.only_in_theirs);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail (module not wired yet, that's fine)**

From `app/src-tauri/`:
```
cargo test graph_diff 2>&1
```
Expected: compilation error about missing module declaration - that's correct, confirms tests are written.

- [ ] **Step 3: Commit graph_diff.rs**

```bash
git add app/src-tauri/src/graph_diff.rs
git commit -m "feat(rust): add graph_diff module with NodeStatus, GraphDiff, and unit tests"
```

---

### Task 4: Rust wiring - schema, ipc, lib, main

**Files:**
- Modify: `app/src-tauri/src/schema.rs`
- Modify: `app/src-tauri/src/ipc.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Modify: `app/src-tauri/src/main.rs`

- [ ] **Step 1: Add `graphs` field to `Asset` in schema.rs**

In `app/src-tauri/src/schema.rs`, add `use std::collections::HashMap;` at the top (after the `use serde` line), then add the `graphs` field to `Asset`:

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// ... (keep existing code) ...

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub class: String,

    #[serde(rename = "parentClass", default)]
    pub parent_class: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub properties: Vec<Property>,

    #[serde(default)]
    pub graphs: Option<HashMap<String, String>>,
}
```

- [ ] **Step 2: Add `diff_graphs` to ipc.rs**

At the top of `app/src-tauri/src/ipc.rs`, add to existing imports:
```rust
use crate::graph_diff::{GraphDiff, diff_graphs_inner};
```

Then add after the `diff_snapshots` command:

```rust
pub fn diff_graphs_ipc_inner(ours: &AssetSnapshot, theirs: &AssetSnapshot) -> Vec<GraphDiff> {
    let ours_graphs = ours.asset.graphs.as_ref().cloned().unwrap_or_default();
    let theirs_graphs = theirs.asset.graphs.as_ref().cloned().unwrap_or_default();
    diff_graphs_inner(&ours_graphs, &theirs_graphs)
}

#[tauri::command]
pub fn diff_graphs(ours: AssetSnapshot, theirs: AssetSnapshot) -> Vec<GraphDiff> {
    diff_graphs_ipc_inner(&ours, &theirs)
}
```

- [ ] **Step 3: Declare module in lib.rs**

In `app/src-tauri/src/lib.rs`, add the module declaration and re-export:

```rust
pub mod graph_diff;
```

And add to the re-exports line:
```rust
pub use graph_diff::{GraphDiff, NodeStatus};
```

The file should look like:
```rust
//! Backend for unreal-merge.

pub mod app_mode;
pub mod cli;
pub mod diff;
pub mod git;
pub mod graph_diff;
pub mod installer;
pub mod ipc;
pub mod merge;
pub mod schema;
pub mod sidecar;

pub use diff::{PropertyChange, diff_properties};
pub use graph_diff::{GraphDiff, NodeStatus};
pub use schema::{Asset, AssetSnapshot, ErrorResponse, Package, Property, PropertyValue};
pub use sidecar::{Sidecar, SidecarConfig, extract_json_objects};
```

- [ ] **Step 4: Register `diff_graphs` in main.rs**

In `app/src-tauri/src/main.rs`, add `unreal_merge::ipc::diff_graphs` to the `generate_handler!` macro:

```rust
.invoke_handler(tauri::generate_handler![
    unreal_merge::ipc::get_app_mode,
    unreal_merge::ipc::diff_snapshots,
    unreal_merge::ipc::apply_resolution,
    unreal_merge::ipc::export_asset,
    unreal_merge::ipc::close_with_exit,
    unreal_merge::ipc::diff_graphs,
])
```

- [ ] **Step 5: Run Rust tests to verify graph_diff unit tests pass**

From `app/src-tauri/`:
```
cargo test graph_diff
```
Expected output:
```
running 6 tests
test graph_diff::tests::test_diff_added ... ok
test graph_diff::tests::test_diff_changed ... ok
test graph_diff::tests::test_diff_removed ... ok
test graph_diff::tests::test_diff_unchanged ... ok
test graph_diff::tests::test_graph_only_in_ours ... ok
test graph_diff::tests::test_graph_only_in_theirs ... ok
test result: ok. 6 passed
```

- [ ] **Step 6: Verify Rust compiles cleanly**

```
cargo check
```
Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add app/src-tauri/src/schema.rs app/src-tauri/src/ipc.rs app/src-tauri/src/lib.rs app/src-tauri/src/main.rs
git commit -m "feat(rust): wire diff_graphs IPC command - schema graphs field, ipc, lib, main"
```

---

### Task 5: Frontend - install ueblueprint, update types, ipc, CSS

**Files:**
- Modify: `app/package.json` (via pnpm)
- Modify: `app/src/main.tsx`
- Modify: `app/src/types.ts`
- Modify: `app/src/ipc.ts`
- Modify: `app/src/styles.css`

- [ ] **Step 1: Install ueblueprint package**

From the `app/` directory:
```
pnpm add ueblueprint
```
Expected: `ueblueprint` appears in `dependencies` in `package.json`.

- [ ] **Step 2: Import ueblueprint CSS in main.tsx**

In `app/src/main.tsx`, add after the existing `import "./styles.css";` line:

```ts
import "ueblueprint/dist/css/ueb-style.min.css";
```

The full file should be:
```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";
import "ueblueprint/dist/css/ueb-style.min.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

- [ ] **Step 3: Add GraphDiff, NodeStatus, and graphs to types.ts**

In `app/src/types.ts`, add after the `PropertyChange` type:

```ts
export type NodeStatus = "added" | "removed" | "changed" | "unchanged";

export interface GraphDiff {
  name: string;
  onlyInOurs: boolean;
  onlyInTheirs: boolean;
  nodeStatuses: Record<string, NodeStatus>;
}
```

And update the `Asset` interface to add `graphs`:

```ts
export interface Asset {
  class: string;
  parentClass: string;
  name: string;
  properties: Property[];
  graphs?: Record<string, string>;
}
```

- [ ] **Step 4: Add diffGraphs to ipc.ts**

In `app/src/ipc.ts`, add import and function:

```ts
import type { AppMode, AssetSnapshot, PropertyChange, GraphDiff } from "./types";
```

And add the function after `diffSnapshots`:

```ts
export async function diffGraphs(
  ours: AssetSnapshot,
  theirs: AssetSnapshot,
): Promise<GraphDiff[]> {
  return invoke<GraphDiff[]>("diff_graphs", { ours, theirs });
}
```

- [ ] **Step 5: Add diff CSS classes to styles.css**

Append to `app/src/styles.css`:

```css
.uem-diff-added {
  outline: 2px solid #2d8a4e !important;
  box-shadow: 0 0 12px rgba(45, 138, 78, 0.5) !important;
}
.uem-diff-removed {
  outline: 2px solid #8a2d2d !important;
  box-shadow: 0 0 12px rgba(138, 45, 45, 0.5) !important;
}
.uem-diff-changed {
  outline: 2px solid #8a742d !important;
  box-shadow: 0 0 12px rgba(138, 116, 45, 0.5) !important;
}
```

- [ ] **Step 6: TypeScript check**

From `app/`:
```
pnpm tsc --noEmit
```
Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add app/package.json app/pnpm-lock.yaml app/src/main.tsx app/src/types.ts app/src/ipc.ts app/src/styles.css
git commit -m "feat(frontend): add ueblueprint dep, GraphDiff types, diffGraphs IPC, diff CSS classes"
```

---

### Task 6: graphDiff.ts - DOM overlay function

**Files:**
- Create: `app/src/graphDiff.ts`

The `ueblueprint` library renders nodes as `<ueb-node>` custom elements. NodeGuid is NOT a DOM data attribute - it is accessed via the `entity` JS property: `el.entity.NodeGuid.toString()`.

- [ ] **Step 1: Create app/src/graphDiff.ts**

```ts
import type { GraphDiff, NodeStatus } from "./types";

export function applyDiffOverlay(
  container: HTMLElement,
  diff: GraphDiff,
  side: "ours" | "theirs",
): void {
  const nodeEls = container.querySelectorAll("ueb-node");
  nodeEls.forEach((el) => {
    const nodeEl = el as HTMLElement & { entity?: { NodeGuid?: { toString(): string } } };
    const guid = nodeEl.entity?.NodeGuid?.toString();
    if (!guid) return;

    const status: NodeStatus | undefined = diff.nodeStatuses[guid];
    if (!status || status === "unchanged") return;

    if (status === "added" && side === "theirs") {
      nodeEl.classList.add("uem-diff-added");
    } else if (status === "removed" && side === "ours") {
      nodeEl.classList.add("uem-diff-removed");
    } else if (status === "changed") {
      nodeEl.classList.add("uem-diff-changed");
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
git commit -m "feat(frontend): add applyDiffOverlay - injects diff CSS on ueb-node elements by entity.NodeGuid"
```

---

### Task 7: GraphPane component

**Files:**
- Create: `app/src/views/GraphPane.tsx`
- Create: `app/src/views/GraphPane.module.css`

`GraphPane` renders a single `<ueb-blueprint>` web component using pure DOM manipulation in `useEffect` (not JSX), then applies the diff overlay after render.

- [ ] **Step 1: Create GraphPane.module.css**

```css
.pane {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-right: 1px solid #2a2a4a;
  min-width: 0;
}

.pane:last-child {
  border-right: none;
}

.label {
  background: #12121e;
  border-bottom: 1px solid #2a2a4a;
  padding: 6px 12px;
  font-size: 11px;
  font-weight: 600;
  flex-shrink: 0;
  color: #9fb0ca;
}

.label.ours {
  color: #7fca9f;
}

.canvas {
  flex: 1;
  overflow: hidden;
  position: relative;
}

.canvas ueb-blueprint {
  width: 100%;
  height: 100%;
  display: block;
}

.empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: #555;
  font-size: 12px;
  font-style: italic;
}
```

- [ ] **Step 2: Create GraphPane.tsx**

```tsx
import { useEffect, useRef } from "react";
import type { GraphDiff } from "../types";
import { applyDiffOverlay } from "../graphDiff";
import styles from "./GraphPane.module.css";

interface Props {
  label: string;
  side: "ours" | "theirs";
  graphText: string | undefined;
  diff: GraphDiff | undefined;
}

export default function GraphPane({ label, side, graphText, diff }: Props) {
  const canvasRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    canvas.innerHTML = "";

    if (!graphText) return;

    const blueprintEl = document.createElement("ueb-blueprint");
    blueprintEl.style.width = "100%";
    blueprintEl.style.height = "100%";
    blueprintEl.style.display = "block";

    const templateEl = document.createElement("template");
    templateEl.innerHTML = graphText;
    blueprintEl.appendChild(templateEl);
    canvas.appendChild(blueprintEl);

    if (diff) {
      requestAnimationFrame(() => {
        applyDiffOverlay(canvas, diff, side);
      });
    }
  }, [graphText, diff, side]);

  return (
    <div className={styles.pane}>
      <div className={`${styles.label} ${side === "ours" ? styles.ours : ""}`}>
        {label}
      </div>
      <div ref={canvasRef} className={styles.canvas}>
        {!graphText && <div className={styles.empty}>No graph data</div>}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: TypeScript check**

From `app/`:
```
pnpm tsc --noEmit
```
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add app/src/views/GraphPane.tsx app/src/views/GraphPane.module.css
git commit -m "feat(frontend): add GraphPane component - ueb-blueprint DOM rendering with diff overlay"
```

---

### Task 8: GraphView component - two-pane layout with graph switcher

**Files:**
- Create: `app/src/views/GraphView.tsx`
- Create: `app/src/views/GraphView.module.css`

`GraphView` shows both `GraphPane`s side by side, plus a graph-name dropdown that lists all graphs from the union of ours/theirs. Default: `"EventGraph"` if present, else first alphabetically.

- [ ] **Step 1: Create GraphView.module.css**

```css
.container {
  display: flex;
  flex-direction: column;
  flex: 1;
  overflow: hidden;
  min-height: 0;
}

.toolbar {
  background: #0d0d1a;
  border-bottom: 1px solid #2a2a4a;
  padding: 4px 12px;
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
  font-size: 11px;
  color: #888;
}

.graphSelect {
  background: #1a1a2e;
  border: 1px solid #2a2a4a;
  color: #ccc;
  padding: 3px 8px;
  border-radius: 3px;
  font-size: 11px;
}

.badge {
  font-size: 9px;
  padding: 1px 5px;
  border-radius: 8px;
  font-weight: 700;
}

.badgeOurs {
  background: #4d1e1e;
  color: #ca7f7f;
}

.badgeTheirs {
  background: #1e2e4d;
  color: #9fb0ca;
}

.split {
  display: flex;
  flex: 1;
  overflow: hidden;
  min-height: 0;
}
```

- [ ] **Step 2: Create GraphView.tsx**

```tsx
import { useMemo, useState } from "react";
import type { AssetSnapshot, GraphDiff } from "../types";
import GraphPane from "./GraphPane";
import styles from "./GraphView.module.css";

interface Props {
  ours: AssetSnapshot;
  theirs: AssetSnapshot;
  graphDiffs: GraphDiff[];
}

export default function GraphView({ ours, theirs, graphDiffs }: Props) {
  const allGraphNames = useMemo(() => {
    const names = new Set<string>([
      ...Object.keys(ours.asset.graphs ?? {}),
      ...Object.keys(theirs.asset.graphs ?? {}),
    ]);
    const sorted = Array.from(names).sort();
    // Put EventGraph first if present
    const eventIdx = sorted.indexOf("EventGraph");
    if (eventIdx > 0) {
      sorted.splice(eventIdx, 1);
      sorted.unshift("EventGraph");
    }
    return sorted;
  }, [ours, theirs]);

  const [activeGraph, setActiveGraph] = useState<string>(
    () => allGraphNames[0] ?? "",
  );

  const activeDiff = useMemo(
    () => graphDiffs.find((d) => d.name === activeGraph),
    [graphDiffs, activeGraph],
  );

  const oursText = ours.asset.graphs?.[activeGraph];
  const theirsText = theirs.asset.graphs?.[activeGraph];

  const onlyInOurs = activeDiff?.onlyInOurs ?? (oursText != null && theirsText == null);
  const onlyInTheirs = activeDiff?.onlyInTheirs ?? (oursText == null && theirsText != null);

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
          <span className={`${styles.badge} ${styles.badgeOurs}`}>
            only in Ours
          </span>
        )}
        {onlyInTheirs && (
          <span className={`${styles.badge} ${styles.badgeTheirs}`}>
            only in Theirs
          </span>
        )}
      </div>
      <div className={styles.split}>
        <GraphPane
          label="Ours"
          side="ours"
          graphText={oursText}
          diff={activeDiff}
        />
        <GraphPane
          label="Theirs"
          side="theirs"
          graphText={theirsText}
          diff={activeDiff}
        />
      </div>
    </div>
  );
}
```

- [ ] **Step 3: TypeScript check**

From `app/`:
```
pnpm tsc --noEmit
```
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add app/src/views/GraphView.tsx app/src/views/GraphView.module.css
git commit -m "feat(frontend): add GraphView - side-by-side graph panes with graph switcher dropdown"
```

---

### Task 9: Diff.tsx - Graph/Properties tab switcher

**Files:**
- Modify: `app/src/views/Diff.tsx`
- Modify: `app/src/views/Diff.module.css`

`Diff.tsx` gets a tab row (Graph | Properties) when the asset is a Blueprint. For non-Blueprints, the Properties view is shown directly with no tab row.

- [ ] **Step 1: Read current Diff.module.css to know existing class names**

Read `app/src/views/Diff.module.css` before editing.

- [ ] **Step 2: Add tab CSS to Diff.module.css**

Append to `app/src/views/Diff.module.css`:

```css
.tabRow {
  display: flex;
  gap: 2px;
  padding: 6px 12px 0;
  background: #0d0d1a;
  border-bottom: 1px solid #2a2a4a;
  flex-shrink: 0;
}

.tab {
  padding: 5px 16px;
  border-radius: 4px 4px 0 0;
  font-size: 12px;
  cursor: pointer;
  border: 1px solid transparent;
  border-bottom: none;
  color: #888;
  background: transparent;
  user-select: none;
}

.tab:hover {
  color: #ccc;
}

.tabActive {
  background: #1d1f23;
  border-color: #2a2a4a;
  color: #ccc;
}
```

- [ ] **Step 3: Update Diff.tsx**

Replace the contents of `app/src/views/Diff.tsx` with:

```tsx
import { useEffect, useMemo, useState } from "react";
import { applyResolution, closeWithExit, diffGraphs, diffSnapshots, exportAsset } from "../ipc";
import type { AssetSnapshot, GraphDiff, PropertyChange } from "../types";
import GraphView from "./GraphView";
import PropertiesDiff from "./PropertiesDiff";
import Resolve from "./Resolve";
import styles from "./Diff.module.css";

interface Props {
  oursPath: string;
  theirsPath: string;
  destPath: string;
}

type Status =
  | { kind: "loading" }
  | { kind: "error"; message: string }
  | {
      kind: "ready";
      ours: AssetSnapshot;
      theirs: AssetSnapshot;
      changes: PropertyChange[];
      graphDiffs: GraphDiff[];
    };

type Tab = "graph" | "properties";

export default function Diff({ oursPath, theirsPath, destPath }: Props) {
  const [status, setStatus] = useState<Status>({ kind: "loading" });
  const [resolving, setResolving] = useState(false);
  const [activeTab, setActiveTab] = useState<Tab>("graph");

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const [ours, theirs] = await Promise.all([
          exportAsset(oursPath),
          exportAsset(theirsPath),
        ]);
        const [changes, graphDiffs] = await Promise.all([
          diffSnapshots(ours, theirs),
          diffGraphs(ours, theirs),
        ]);
        if (!cancelled)
          setStatus({ kind: "ready", ours, theirs, changes, graphDiffs });
      } catch (e) {
        if (!cancelled) setStatus({ kind: "error", message: String(e) });
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, [oursPath, theirsPath]);

  const changedPaths = useMemo(() => {
    if (status.kind !== "ready") return new Set<string>();
    const s = new Set<string>();
    for (const c of status.changes) s.add(c.path);
    return s;
  }, [status]);

  async function resolve(kind: "ours" | "theirs" | "abort") {
    setResolving(true);
    try {
      if (kind === "abort") {
        await closeWithExit(1);
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

  const isBlueprint = status.ours.asset.class === "Blueprint";

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
          <div
            className={`${styles.tab} ${activeTab === "graph" ? styles.tabActive : ""}`}
            onClick={() => setActiveTab("graph")}
          >
            Graph
          </div>
          <div
            className={`${styles.tab} ${activeTab === "properties" ? styles.tabActive : ""}`}
            onClick={() => setActiveTab("properties")}
          >
            Properties
          </div>
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
        />
      )}

      <Resolve
        onTakeOurs={() => resolve("ours")}
        onTakeTheirs={() => resolve("theirs")}
        onAbort={() => resolve("abort")}
        disabled={resolving}
      />
    </div>
  );
}
```

- [ ] **Step 4: TypeScript check**

From `app/`:
```
pnpm tsc --noEmit
```
Expected: no errors.

- [ ] **Step 5: Run Rust tests one final time**

From `app/src-tauri/`:
```
cargo test
```
Expected: all tests pass (including graph_diff's 6 tests).

- [ ] **Step 6: Commit**

```bash
git add app/src/views/Diff.tsx app/src/views/Diff.module.css
git commit -m "feat(frontend): add Graph/Properties tab switcher to Diff - Blueprint assets default to Graph view"
```

---

## Post-Implementation Verification

After all tasks complete, verify end-to-end with the dev server:

```
cd app && pnpm tauri dev -- -- --git-driver ancestor.uasset ours.uasset theirs.uasset BP_MinimalChar.uasset
```

Expected: the diff window opens, shows a "Graph" tab and a "Properties" tab in the header. Clicking "Graph" renders the ueblueprint web component side-by-side. Changed/added/removed nodes should have colored outlines (green/red/yellow).
