# Blueprint Graph View Design Spec

**Date:** 2026-05-23
**Feature:** Plan 4 — Blueprint graph view in the merge diff UI

---

## Goal

Add a Blueprint graph view to the existing Tauri diff window. When a `.uasset` is a Blueprint, the user sees the actual node graph (nodes, pins, wires) rendered side-by-side — Ours on the left, Theirs on the right — with changed/added/removed nodes highlighted. A tab switcher lets them toggle between Graph and Properties views.

---

## Architecture Overview

The feature adds three layers on top of the existing Plan 1–3 stack:

1. **C++ commandlet extension** — `BlueprintExporter.cpp` exports raw UE serialization text per graph using `FBlueprintEditorUtils::ExportNodesToText`. Same editor API UE uses for copy-paste. Returns one text blob per graph name.

2. **Rust schema + diff** — `graph_texts: Option<HashMap<String, String>>` added to `Asset`. A new `diff_graphs` IPC command compares GUIDs across Ours/Theirs and returns diff status per node GUID (`added | removed | changed | unchanged`).

3. **Frontend rendering** — `ueblueprint` npm package (`npm i ueblueprint`) renders `<ueb-blueprint>` web components. Pixel-identical to UE's Blueprint editor. Diff overlay is injected into the rendered DOM post-render by matching node GUID attributes.

---

## Data Pipeline

### 1. UE Commandlet (`BlueprintExporter.cpp`)

Called from `AssetExporter.cpp` after the properties walk, when `Asset->IsA<UBlueprint>()`:

```cpp
// BlueprintExporter.h
TArray<FGraphExport> ExportGraphs(UBlueprint* Blueprint);

struct FGraphExport {
    FString GraphName;   // "EventGraph", "MyFunction", etc.
    FString GraphText;   // raw UE serialization text (Begin Object...End Object)
};
```

Implementation walks `UBlueprint::UbergraphPages`, `FunctionGraphs`, and `MacroGraphs`. For each `UEdGraph`:

```cpp
TSet<UEdGraphNode*> NodeSet(Graph->Nodes);
FString ExportedText;
FBlueprintEditorUtils::ExportNodesToText(NodeSet, ExportedText);
```

Result is appended to the JSON response as:

```json
{
  "ok": true,
  "asset": {
    "class": "Blueprint",
    "graphs": {
      "EventGraph": "Begin Object Class=...\nEnd Object\n...",
      "UpdateMovement": "Begin Object Class=...\nEnd Object\n..."
    }
  }
}
```

Non-Blueprint assets get `"graphs": null`. No change to existing property export path.

`MergeBinariesExport.Build.cs` must add `"UnrealEd"` to `PrivateDependencyModuleNames` (required for `FBlueprintEditorUtils`). It is already an editor-only plugin so this is safe.

### 2. Rust Schema (`schema.rs`)

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Asset {
    pub class: String,
    pub name: String,
    pub parent_class: Option<String>,
    pub properties: Vec<Property>,
    pub graphs: Option<HashMap<String, String>>,  // graph name → UE text
}
```

### 3. New IPC command: `diff_graphs`

```rust
// ipc.rs
#[tauri::command]
pub fn diff_graphs(
    ours: Asset,
    theirs: Asset,
) -> Vec<GraphDiff>
```

`GraphDiff` is defined in a new `graph_diff.rs`:

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GraphDiff {
    pub name: String,
    pub only_in_ours: bool,    // graph exists in ours but not theirs
    pub only_in_theirs: bool,  // graph exists in theirs but not ours
    pub node_statuses: HashMap<String, NodeStatus>,  // GUID → status
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum NodeStatus {
    Added,    // GUID in theirs only
    Removed,  // GUID in ours only
    Changed,  // GUID in both, node text differs
    Unchanged,
}
```

GUID extraction: parse `NodeGuid=XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX` from the UE text with a regex. Node text comparison: after splitting on `Begin Object` / `End Object` boundaries, compare per-node text blobs between ours and theirs for same-GUID nodes.

---

## Frontend Components

### npm dependency

```json
"ueblueprint": "latest"
```

CSS import in `main.tsx`:
```ts
import 'ueblueprint/dist/css/ueb-style.min.css'
```

### New files

```
app/src/views/GraphView.tsx         — tab switcher + two GraphPane side by side
app/src/views/GraphPane.tsx         — single <ueb-blueprint> wrapper + diff overlay
app/src/graphDiff.ts                — applies diff CSS classes to rendered DOM nodes
app/src/types.ts                    — add GraphDiff, NodeStatus
app/src/ipc.ts                      — add diffGraphs()
```

### `GraphView.tsx`

```tsx
// Tab row: [Graph] [Properties]
// Graph tab (default for Blueprint assets):
//   <div class="graph-split">
//     <GraphPane label="Ours"   text={oursGraphs[activeGraph]}  diffs={graphDiffs[activeGraph]} />
//     <GraphPane label="Theirs" text={theirsGraphs[activeGraph]} diffs={graphDiffs[activeGraph]} />
//   </div>
// Graph switcher dropdown: lists all graph names from union of ours+theirs graphs
```

`Diff.tsx` renders `<GraphView>` when `asset.class === "Blueprint"`, otherwise falls through to the existing properties-only layout.

### `GraphPane.tsx`

```tsx
// Renders a single <ueb-blueprint> web component.
// After mount, calls applyDiffOverlay(containerRef, diffs, side).
//
// Props:
//   graphText: string           — raw UE Begin Object text
//   diffs: GraphDiff            — node GUID → NodeStatus
//   side: "ours" | "theirs"
//   label: string

function GraphPane({ graphText, diffs, side, label }) {
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    // ueblueprint renders synchronously on web component upgrade
    // give it one animation frame then apply overlay
    requestAnimationFrame(() => applyDiffOverlay(ref.current, diffs, side))
  }, [graphText, diffs])

  return (
    <div ref={ref} class="graph-pane">
      <div class="pane-label">{label}</div>
      <ueb-blueprint style="--ueb-height: 100%">
        <template dangerouslySetInnerHTML={{ __html: graphText }} />
      </ueb-blueprint>
    </div>
  )
}
```

### `graphDiff.ts` — DOM overlay

`ueblueprint` renders each node as a DOM element with a `data-node-guid` attribute (or similar — confirmed by inspecting the library's output). After render, query those elements and apply diff border classes:

```ts
export function applyDiffOverlay(
  container: HTMLElement,
  diffs: GraphDiff,
  side: "ours" | "theirs"
) {
  for (const [guid, status] of Object.entries(diffs.node_statuses)) {
    const el = container.querySelector(`[data-node-guid="${guid}"]`)
    if (!el) continue
    if (status === "added" && side === "theirs")
      el.classList.add("uem-diff-added")
    if (status === "removed" && side === "ours")
      el.classList.add("uem-diff-removed")
    if (status === "changed")
      el.classList.add("uem-diff-changed")
  }
}
```

CSS added to `index.css`:
```css
.uem-diff-added   { outline: 2px solid #2d8a4e !important; box-shadow: 0 0 12px rgba(45,138,78,0.5) !important; }
.uem-diff-removed { outline: 2px solid #8a2d2d !important; box-shadow: 0 0 12px rgba(138,45,45,0.5) !important; }
.uem-diff-changed { outline: 2px solid #8a742d !important; box-shadow: 0 0 12px rgba(138,116,45,0.5) !important; }
```

---

## Graph Switcher UI

Graph tab strip below the main Graph/Properties tab row. Lists all graph names from the union of `Object.keys(oursAsset.graphs ?? {})` and `Object.keys(theirsAsset.graphs ?? {})`. Default selection: `"EventGraph"` if present, otherwise first alphabetically.

Graphs present in only one side show a badge: `● only in Ours` / `● only in Theirs`.

---

## ueblueprint DOM attribute for node GUIDs

The exact DOM attribute name needs to be confirmed by inspecting the library at runtime. Two verification approaches:

1. Render a known `Begin Object` blob in the browser console and inspect the resulting DOM.
2. Check `ueblueprint`'s source: look for where it attaches node identity to DOM elements.

If ueblueprint doesn't expose GUIDs as DOM attributes, fallback: parse the node title string from the UE text and query `[data-name="..."]` or similar. Plan task includes a discovery step before implementing `applyDiffOverlay`.

---

## TypeScript types (`types.ts` additions)

```ts
export type NodeStatus = "added" | "removed" | "changed" | "unchanged"

export interface GraphDiff {
  name: string
  onlyInOurs: boolean
  onlyInTheirs: boolean
  nodeStatuses: Record<string, NodeStatus>
}
```

`Asset` gets `graphs?: Record<string, string>`.

---

## Testing

### Rust unit tests (`graph_diff.rs`)

- `test_diff_added`: theirs has a node GUID ours doesn't → status `added`
- `test_diff_removed`: ours has a node GUID theirs doesn't → status `removed`
- `test_diff_changed`: same GUID, different pin count in text → status `changed`
- `test_diff_unchanged`: identical nodes → status `unchanged`
- `test_graph_only_in_ours`: graph present in ours, absent in theirs

Test fixture strings are minimal `Begin Object...End Object` blobs, not full UE output.

### Commandlet golden test

Extend `tools/golden-test.ps1` to capture `graphs` field from `BP_MinimalChar.uasset`. The v1 and v2 golden files gain a `graphs.EventGraph` key with the raw UE text. Bless once, verify on subsequent runs.

### Frontend

`GraphPane.tsx` is tested with a minimal mock `ueblueprint` web component that renders a div per `NodeGuid` found in the template text. `applyDiffOverlay` is unit-tested against a synthetic DOM with known GUIDs.

---

## Files Changed / Created

| File | Change |
|---|---|
| `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/BlueprintExporter.cpp` | **New** — graph export |
| `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/BlueprintExporter.h` | **New** |
| `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/AssetExporter.cpp` | Modify — call BlueprintExporter when class is Blueprint |
| `app/src-tauri/src/schema.rs` | Add `graphs` field to `Asset` |
| `app/src-tauri/src/graph_diff.rs` | **New** — `GraphDiff`, `NodeStatus`, `diff_graphs_inner` |
| `app/src-tauri/src/ipc.rs` | Add `diff_graphs` command |
| `app/src-tauri/src/lib.rs` | Declare `graph_diff` module |
| `app/src-tauri/src/main.rs` | Register `diff_graphs` in `generate_handler!` |
| `app/package.json` | Add `ueblueprint` dependency |
| `app/src/main.tsx` | Import `ueb-style.min.css` |
| `app/src/types.ts` | Add `GraphDiff`, `NodeStatus`, `graphs` to `Asset` |
| `app/src/ipc.ts` | Add `diffGraphs()` |
| `app/src/views/GraphView.tsx` | **New** |
| `app/src/views/GraphPane.tsx` | **New** |
| `app/src/graphDiff.ts` | **New** |
| `app/src/index.css` | Add `.uem-diff-*` classes |
| `app/src/views/Diff.tsx` | Add Graph/Properties tab switcher |
| `Examples/graph-v1.expected.json` | **New** golden |
| `Examples/graph-v2.expected.json` | **New** golden |
