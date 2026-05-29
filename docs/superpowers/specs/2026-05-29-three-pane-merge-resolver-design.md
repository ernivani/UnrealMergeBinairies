# Three-Pane Merge Resolver (JetBrains-style) — Design

**Date:** 2026-05-29
**Status:** Approved, building

## Goal

Replace the current 2-pane GraphView (Ours | Theirs with floating per-node conflict
pickers) with a JetBrains-style 3-way resolver: **Ours graph (left) | Result change-list
(center) | Theirs graph (right)**. The center is the interactive "what to keep" surface;
unchanged nodes are implicitly kept as the common base. Non-conflicting changes are
auto-included (clean merges need zero clicks); only conflicts demand attention.

## Scope

Frontend only. The backend (`buildMergedGraphs`, `selections`, `applyGraphMerge`,
three-way diff) already supports per-node `ours`/`theirs`/`skip` selection — no Rust/UE
changes. This is a presentation/interaction redesign.

## Layout

```
┌─ Ours (graph) ─┐ ┌─ RESULT ───────────────────┐ ┌─ Theirs (graph) ─┐
│ diff outlines  │ │ <graph name>               │ │ diff outlines    │
│ selected row's │ │  + Print "zero"   [Keep|×] │ │ selected row's   │
│ node flashes   │ │  ~ SET Health     [O | T]  │ │ node flashes     │
│                │ │  ! Branch  ⚠      [O|T|×]  │ │                  │
└────────────────┘ │ 6 changes · 1 conflict     │ └──────────────────┘
  [graph dropdown] └────────────────────────────┘   [Take Both]
```

3-column flex inside GraphView's `.split`. The graph dropdown (existing) switches which
graph all three columns show. Take Both stays in the bottom Resolve bar.

## Center Result panel (`ResultPanel.tsx`)

Lists only the **changed** nodes for the active graph (unchanged = kept as base, not
listed). One row per changed NodeGuid:

| Status (ThreeWayNodeStatus) | Row controls | Default | MergeSide mapping |
|---|---|---|---|
| addedInOurs / addedInTheirs / addedInBoth | `[Keep \| Skip]` | Keep | Keep→ours(or theirs for addedInTheirs), Skip→skip |
| removedInOurs / removedInTheirs | `[Keep \| Drop]` | Keep | Keep→side that still has it, Drop→skip |
| modifiedInOurs / modifiedInTheirs | `[Ours \| Theirs]` | the changed side | direct |
| modifiedInBoth / addedInBothConflict / modifyDeleteConflict | `[Ours \| Theirs \| Skip]` ⚠ | Ours | direct |
| unchanged / removedInBoth | not listed | — | — |

Row label: human-readable node name parsed from the node blob — prefer the function
name (`MemberName="X"`), else variable name, else the `K2Node_*` class shortened
(e.g. `K2Node_VariableSet` → "SET", `K2Node_IfThenElse` → "Branch"). Falls back to the
GUID tail. A small status glyph/color precedes it (green +, red −, amber ~, magenta !).

Header: `<N> change(s) · <M> conflict(s)` (M in magenta; "no conflicts" when 0).

Selecting a row sets a `selectedGuid` (local to GraphView) → both side panes add a
`uem-selected` flash/outline class to the matching `ueb-node`.

## Component changes

- **New** `app/src/views/ResultPanel.tsx` + `ResultPanel.module.css` — pure presentational;
  props: `{ diff: ThreeWayGraphDiff, ours/theirs/ancestorGraphs (text), selections, onSelect(guid,side), selectedGuid, onRowClick(guid) }`. Computes the change rows from `diff.nodeStatuses` + node labels.
- **New** `app/src/nodeLabel.ts` — `nodeLabel(blob: string): string` (+ small unit-testable pure fn). Used by ResultPanel.
- **Modify** `GraphView.tsx` — 3-column layout; drop `ConflictPickers`; own `selectedGuid` state; pass `selectedGuid` to both `GraphPane`s.
- **Modify** `GraphPane.tsx` — accept optional `selectedGuid`; after overlay, add `uem-selected` to the matching node.
- **Modify** `graphDiff.ts` / `styles.css` — add `.uem-selected` flash style.
- **Modify** `BlueprintTest.tsx` — render the new 3-pane (already passes threeWayDiffs).
- **Reuse** `mergeGraphs.ts` `defaultSide`, `parseNodeBlobs`; `Diff.tsx` selection state and Take Both unchanged.

## Testing

- `nodeLabel` pure-function cases (function ref, variable ref, knot, event) — vitest if
  configured, else a tiny inline check; at minimum `pnpm tsc --noEmit` clean.
- Manual: `pnpm dev` → BlueprintTest shows 3 panes; toggling a conflict updates the
  counter and dims the opposite side; clicking a row flashes the node.
- Full Tauri: real conflict renders the list; Take Both still writes the merged asset.

## Non-goals

- Live-rerendering a third "result" graph canvas (rejected for cost/risk on big graphs).
- Editing node internals. Resolution is per-node side selection only.
