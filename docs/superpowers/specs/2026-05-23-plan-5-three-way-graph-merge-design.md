# Plan 5 - Three-Way Graph Merge ("Take Both") Design

**Date:** 2026-05-23
**Status:** Draft, ready for implementation

## Goal

Enable VS Code-style "accept current AND incoming" for Blueprint `.uasset` files in the merge driver UI. When the user clicks **Take Both**, the tool produces a merged binary that contains the non-conflicting graph-node changes from *both* `ours` and `theirs`, plus the user's explicit pick for any conflicting nodes.

## Non-Goals

- **Wire/pin-level merge.** This design merges at node granularity. If accepted nodes from opposite sides reference pin IDs on a removed/replaced node, the resulting graph may have dangling `LinkedTo` references. Surfacing this is a stretch goal; auto-fixing it is out of scope.
- **Property-level three-way merge.** Plan 5 only adds three-way merge for the graph view. The properties tab keeps its existing two-way diff and Take Ours / Take Theirs / Abort buttons.
- **Non-Blueprint asset types.** Only `Asset.class == "Blueprint"` shows the Take Both button.
- **Re-targeting wires** that point to a removed-from-ancestor node. The C++ merge attempts a literal node import; if UE rejects it, we surface the error to the user.

## Architecture

```
ancestor.uasset ─┐
ours.uasset     ─┼─► export (existing) ─► AssetSnapshot ×3
theirs.uasset   ─┘                          │
                                            ▼
                              diff_graphs_three_way (new Rust)
                                            │
                                            ▼
                              ThreeWayGraphDiff[] per graph
                                            │
                          ┌─────────────────┴─────────────────┐
                          ▼                                   ▼
              GraphView renders both panes        User picks per-conflict-node
              with overlay badges                            │
                                                            ▼
                                          buildMergedGraphs(selections)
                                                            │
                                                            ▼
                          apply_graph_merge IPC ──► commandlet `merge` cmd
                                                            │
                                                            ▼
                                          UE rewrites .uasset, saves package
                                                            │
                                                            ▼
                                          Rust copies result → dest
```

## New Wire Types

### `ThreeWayNodeStatus` (Rust enum, serde lowercase)

```rust
pub enum ThreeWayNodeStatus {
    Unchanged,
    ModifiedInOurs,
    ModifiedInTheirs,
    ModifiedInBoth,        // CONFLICT
    AddedInOurs,
    AddedInTheirs,
    AddedInBoth,           // auto if identical, else conflict - see below
    AddedInBothConflict,   // CONFLICT
    RemovedInOurs,
    RemovedInTheirs,
    RemovedInBoth,
    ModifyDeleteConflict,  // CONFLICT
}
```

Wire format (lowercase): `"unchanged"`, `"modifiedInOurs"`, ... (Rust `rename_all = "camelCase"` for variants, plus `lowercase` doesn't fit multi-word; we use `camelCase` - and update [[mock_ue_sidecar]] accordingly.)

> **Decision:** use `#[serde(rename_all = "camelCase")]` on this enum (overriding the existing `lowercase` style on `NodeStatus`). TS mirror is the literal string union.

### `ThreeWayGraphDiff` (Rust struct)

```rust
pub struct ThreeWayGraphDiff {
    pub name: String,
    pub only_in_ours: bool,
    pub only_in_theirs: bool,
    pub only_in_ancestor: bool,
    pub node_statuses: HashMap<String, ThreeWayNodeStatus>,
}
```

### `GraphMergeSelection` (frontend → Rust)

```ts
type Side = "ours" | "theirs" | "skip";
interface GraphMergeSelection {
  perNode: Record<string /* graphName */, Record<string /* guid */, Side>>;
}
```

`"skip"` means "drop this node from the merged graph entirely" (only valid for conflicting nodes; UI hides it otherwise).

## Backend Changes

### `graph_diff.rs` - extend with three-way

Add `diff_graphs_three_way_inner(ancestor, ours, theirs) -> Vec<ThreeWayGraphDiff>`. Algorithm per GUID across the union of all three node sets:

| ancestor | ours | theirs | result |
|---|---|---|---|
| ✓     | ✓ same | ✓ same | unchanged |
| ✓ A   | ✓ A    | ✓ B    | modifiedInTheirs |
| ✓ A   | ✓ B    | ✓ A    | modifiedInOurs |
| ✓ A   | ✓ B    | ✓ B    | modifiedInOurs (both became B; treat as same change - pick either) |
| ✓ A   | ✓ B    | ✓ C    | modifiedInBoth (CONFLICT) |
| ✓     | ✓      | -      | removedInTheirs |
| ✓     | -      | ✓      | removedInOurs |
| ✓     | -      | -      | removedInBoth |
| ✓ A   | ✓ B    | -      | modifyDeleteConflict |
| ✓ A   | -      | ✓ B    | modifyDeleteConflict |
| -     | ✓      | -      | addedInOurs |
| -     | -      | ✓      | addedInTheirs |
| -     | ✓ same | ✓ same | addedInBoth (auto) |
| -     | ✓ A    | ✓ B    | addedInBothConflict |

If `ancestor` graph text is missing for a given graph name (rare - e.g., graph created on both sides), treat it as `""` for that graph. `only_in_ancestor` mirrors `only_in_ours`/`only_in_theirs` semantics for the graph-level fields.

### `ipc.rs` - two new commands

```rust
#[tauri::command]
pub fn diff_graphs_three_way(
    ancestor: AssetSnapshot,
    ours: AssetSnapshot,
    theirs: AssetSnapshot,
) -> Vec<ThreeWayGraphDiff>;

#[tauri::command]
pub fn apply_graph_merge(
    ancestor_path: String,
    ours_path: String,
    theirs_path: String,
    dest_path: String,
    merged_graphs: HashMap<String, String>,  // graph_name → full UE serialization text
    sidecar_override: Option<String>,
    host_project_override: Option<String>,
) -> Result<(), String>;
```

`apply_graph_merge` flow:
1. Send a single `merge` JSON-RPC request to the commandlet with `{path: ancestor_path, mergedGraphs}`. The commandlet returns `{ok, mergedPath}` - the path to the rewritten asset.
2. Copy `mergedPath` → `dest_path` (reusing the read-only-bit handling already in `merge::apply_resolution`).
3. Best-effort delete `mergedPath` (it's a temp file).

If the commandlet returns `ok: false`, propagate the error string to the frontend.

### `merge.rs` - helper for the temp-file copy

Add `apply_merged_file(merged_path: &Path, dest: &Path) -> Result<()>` that does the read-only-bit dance currently inlined in `apply_resolution`. Refactor `apply_resolution` to use it. (Small bonus refactor - file is small and this removes dup.)

## UE Commandlet Changes

### New `merge` JSON-RPC command

Request:
```json
{"id": N, "cmd": "merge", "path": "C:/...ancestor.uasset",
 "mergedGraphs": {"EventGraph": "Begin Object Class=...\n..."}}
```

Response:
```json
{"id": N, "ok": true, "mergedPath": "C:/Users/.../Temp/unreal-merge-<pid>-<hash>.uasset"}
```

Implementation (`StdioSession.cpp` dispatch + new `MergeApplier.{h,cpp}` in the plugin):

1. **Duplicate the ancestor package** to a temp `.uasset` path under the engine's intermediate dir (`FPaths::ProjectIntermediateDir() / "UnrealMerge"`). Use `UEditorAssetLibrary::DuplicateLoadedAsset` or `StaticDuplicateObject`.
2. **Load the duplicated Blueprint** via `LoadObject<UBlueprint>(...)`.
3. **For each graph in `mergedGraphs`:**
   - Find the matching `UEdGraph*` on the duplicate by name (UbergraphPages + FunctionGraphs + MacroGraphs).
   - **Remove all existing nodes** from the graph (`Graph->Nodes.Empty()` after `Modify`).
   - Call `FEdGraphUtilities::ImportNodesFromText(Graph, MergedText, ImportedNodes)`. Add returned nodes to `Graph->Nodes`. Call `NotifyGraphChanged`.
4. Call `FKismetEditorUtilities::CompileBlueprint(Blueprint, ...)` with `EBlueprintCompileOptions::SkipGarbageCollection` - best-effort; log on failure but proceed.
5. `UPackage::SavePackage(Package, Blueprint, RF_Public | RF_Standalone, *TempPath, GError, nullptr, true, true, SAVE_NoError)`.
6. Return `{ok: true, mergedPath: TempPath}` (or `{ok: false, error: ...}` on any failure).

> **Note:** Graphs not present in `mergedGraphs` are left as-is on the ancestor duplicate. This means callers MUST send every graph they want in the output (the frontend sends all graphs).

### Mock sidecar - new `merge` cmd

For dev-mode parity, `mock_ue_sidecar` adds:
- An `ancestor` fixture: a *third* hand-written EventGraph that represents BP_Base before either branch changed it. Concretely: ours = ancestor + a False-branch PrintString; theirs = ancestor + a MaxHealth getter feeding SET Health. So **ancestor = the common subset of ours and theirs** (BeginPlay → SET Health → Branch → True PrintString, with Knot from Get Health). This makes the mock realistic: each side has one isolated change, no conflict.
- `export` returns the ancestor graph when `path.contains("ancestor")` (we'll extend the existing `is_theirs` check).
- New `merge` cmd handler: writes the concatenation of all `mergedGraphs` values to a temp file, returns the path. (The "asset" the mock writes isn't a real .uasset - it's plain text. The frontend never re-parses it; it's just copied to dest. This is enough to exercise the IPC.)

## Frontend Changes

### `types.ts`

```ts
export type ThreeWayNodeStatus =
  | "unchanged"
  | "modifiedInOurs" | "modifiedInTheirs" | "modifiedInBoth"
  | "addedInOurs" | "addedInTheirs" | "addedInBoth" | "addedInBothConflict"
  | "removedInOurs" | "removedInTheirs" | "removedInBoth"
  | "modifyDeleteConflict";

export interface ThreeWayGraphDiff {
  name: string;
  onlyInOurs: boolean;
  onlyInTheirs: boolean;
  onlyInAncestor: boolean;
  nodeStatuses: Record<string, ThreeWayNodeStatus>;
}

export type MergeSide = "ours" | "theirs" | "skip";
```

A status is **conflicting** iff it ends in `Conflict` or is `modifiedInBoth` / `modifyDeleteConflict`.

### `ipc.ts`

```ts
export async function diffGraphsThreeWay(
  ancestor: AssetSnapshot, ours: AssetSnapshot, theirs: AssetSnapshot,
): Promise<ThreeWayGraphDiff[]>;

export async function applyGraphMerge(
  ancestorPath: string, oursPath: string, theirsPath: string, destPath: string,
  mergedGraphs: Record<string, string>,
): Promise<void>;
```

### `Diff.tsx`

- Accept new `ancestorPath: string` prop (from `gitDriverGui.ancestor` in App.tsx).
- When `ancestorPath` is present AND asset is a Blueprint, also export ancestor and call `diffGraphsThreeWay`. Stored in `Status` as `threeWayDiffs: ThreeWayGraphDiff[]` and `ancestor: AssetSnapshot`.
- Two-way `graphDiffs` still computed (used by Properties highlighting and the two-way fallback when no ancestor).
- Resolve bar gains a fourth button: **Take Both** (Blueprint + 3-way available only). Calls a new `resolve("both")` branch that:
  1. Builds `mergedGraphs` from selections (see GraphView).
  2. `await applyGraphMerge(ancestorPath, oursPath, theirsPath, destPath, mergedGraphs)`.
  3. `await closeWithExit(0)`.

### `GraphView.tsx` (3-way aware)

- New props: `threeWayDiffs?: ThreeWayGraphDiff[]`, `ancestor?: AssetSnapshot`. When provided, switch into 3-way mode:
- Selection state: `selections: Map<GraphName, Map<NodeGuid, MergeSide>>` (one `useState`).
- Default selections (computed in `useMemo` from `threeWayDiffs`):
  - `unchanged` / `removedInBoth` → not in map (no choice needed).
  - `modifiedInOurs` / `addedInOurs` / `removedInOurs` / `addedInBoth` → `"ours"`.
  - `modifiedInTheirs` / `addedInTheirs` / `removedInTheirs` → `"theirs"`.
  - Any conflict → `"ours"` (default).
- Each conflicting status renders a small floating overlay over the node with three buttons: **O / T / -** (the `-` = skip). State is set per (graph, guid).
- Non-conflicting non-unchanged statuses get a small **badge** (no buttons) - informational only.
- The existing two-way diff overlay (red/green/amber outlines) is replaced by a 3-way overlay function (`applyThreeWayOverlay`). Color choices:
  - `addedIn*` → green outline
  - `removedIn*` → red outline
  - `modifiedIn*` (non-conflict) → amber outline
  - Any conflict → bright magenta outline with badge
- **Selection-aware visual feedback:** Nodes whose selection is opposite-side or `skip` get reduced opacity (0.3). E.g. if user picks "theirs" for a conflicting node, the `ours` pane's rendering of that node dims.

### `buildMergedGraphs` (new helper, `app/src/mergeGraphs.ts`)

Pure function taking `(threeWayDiffs, ancestor, ours, theirs, selections)` and returning `Record<GraphName, MergedText>`. Per graph:

1. Start with an empty list of node blobs.
2. For each GUID with status `unchanged`: append the ancestor's blob.
3. For each non-`skip` selection: append the blob from the chosen side (`ours.graphs[name]` parsed, or `theirs.graphs[name]` parsed). For `addedInBoth` (auto), append from `ours`.
4. Re-parse using the same `parse_node_blobs` style logic in TS - OR more simply, do this on the Rust side. **Decision: do it in TS** to keep the API surface narrow. Use a tiny TS port of `parse_node_blobs` (already a 25-line function). Live in `app/src/graphTextParse.ts` with unit tests (vitest).
5. Join blobs with `\n` between them. (UE's `ImportNodesFromText` is whitespace-tolerant.)

The Rust side never sees this logic - it just passes the merged text through to the commandlet.

### Auto-merge of all-non-conflict case

If `threeWayDiffs` has zero conflicts, **Take Both** is still useful (it merges the two sides without conflict). The button is enabled in that case too. If there ARE conflicts, the button label flips to **"Take Both (resolve N conflict(s))"** and is enabled (default picks make it always safe to submit).

### `Resolve.tsx`

Add `onTakeBoth?: () => void` prop (optional - only rendered when in 3-way mode). New button between `Take Theirs` and the spacer.

### `BlueprintTest.tsx` (dev fixture)

Add a third hardcoded graph string (`EVENT_GRAPH_ANCESTOR`, matching mock sidecar's ancestor), and pass it through so `pnpm dev` renders the 3-way view. The test view should pre-populate as if a Blueprint conflict was loaded.

## Testing

**Rust unit tests** (`app/src-tauri/src/graph_diff.rs`):
- All 14 rows of the truth table above → one test per status outcome.
- Empty ancestor for one graph (created-in-both case).
- Graph only in ancestor (removed-in-both).

**Rust IPC tests** (`app/src-tauri/src/ipc.rs`):
- `diff_graphs_three_way_inner` smoke test on a 3-snapshot fixture.

**TS unit tests** (vitest already configured? - check; if not, add it):
- `graphTextParse.parseNodeBlobs` round-trips the BP_Base fixture.
- `buildMergedGraphs` produces the expected merged text for: all-auto, one-conflict-pick-ours, one-conflict-pick-skip.

**E2E (manual):**
- `pnpm dev` → BlueprintTest renders with 3-way overlay badges visible.
- `pnpm tauri dev -- -- --git-driver ancestor ours theirs dest` with mock sidecar: open Take Both → window closes 0, dest file equals what mock wrote.

## Risks / open issues

1. **Wire integrity:** A `LinkedTo=(K2Node_X PinId)` reference may point to a node the user skipped. UE's `ImportNodesFromText` is lenient - unresolved refs become null. Acceptable for v1; surface a warning if any pin's referenced node-GUID is missing from the final set.
2. **Pin ID stability across versions:** Pin IDs are stable per session (set at node creation), so OursA and TheirsA share PinIds if they branched from a common ancestor. But if a node was *re-created* on one side (same GUID? - unlikely; that would change GUID), pin IDs diverge. Out of scope for v1.
3. **`SavePackage` in `-run=` commandlet:** Requires `-AllowCommandletRendering` flag is NOT needed for pure data save, but `RF_Standalone` mark on the BP must survive. If save fails in practice, the commandlet returns ok:false with the GError contents.
4. **Mock sidecar fidelity:** The mock writes plain text, not a real `.uasset`. That's fine for IPC + frontend exercise but means the full round-trip (Take Both → restart UE → see merged Blueprint) only works with a real UE sidecar.

## File map (preview - full list in implementation plan)

**New files:**
- `app/src-tauri/src/graph_diff.rs` - gets new `ThreeWayNodeStatus`, `ThreeWayGraphDiff`, `diff_graphs_three_way_inner` + tests
- `app/src/mergeGraphs.ts` - `buildMergedGraphs` + tiny `parseNodeBlobs` TS port
- `app/src/graphTextParse.ts` (or in `mergeGraphs.ts` - TBD)
- `app/src/views/GraphMergeOverlay.tsx` - small per-node picker overlay component (optional split)
- `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeApplier.{h,cpp}` - new
- `app/src-tauri/tests/three_way_merge_e2e_test.rs` - e2e mock-backed test

**Modified:**
- `app/src-tauri/src/ipc.rs`, `lib.rs`, `main.rs`, `merge.rs`
- `app/src-tauri/src/bin/mock_ue_sidecar.rs` - ancestor fixture + `merge` cmd
- `app/src/types.ts`, `ipc.ts`
- `app/src/views/Diff.tsx`, `GraphView.tsx`, `Resolve.tsx`, `BlueprintTest.tsx`, `App.tsx` (pass ancestorPath)
- `app/src/views/GraphView.module.css`, `Resolve.module.css` (overlay picker + button styles)
- `app/src/graphDiff.ts` - add `applyThreeWayOverlay`
- `ue-host/Plugins/MergeBinariesExport/Source/MergeBinariesExport/Private/StdioSession.cpp` - dispatch `merge`
