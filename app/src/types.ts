/**
 * TypeScript mirrors of the Rust wire types. Hand-written rather than
 * generated to keep Plan 3 simple; if these drift from the Rust side,
 * the integration tests added in Task 11 will surface it.
 */

export interface Package {
  name: string;
  engineVersion: string;
  fileVersionUE5: number;
  savedHash: string;
}

export interface Property {
  path: string;
  type: string;
  value: PropertyValue;
}

// PropertyValue is `#[serde(untagged)]` on the Rust side — could be primitive
// or an object summary for structs/arrays/maps/sets. We model it as `unknown`
// at the type-system level and let the rendering layer branch.
export type PropertyValue = unknown;

export interface Asset {
  class: string;
  parentClass: string;
  name: string;
  properties: Property[];
  graphs?: Record<string, string>;
}

export interface AssetSnapshot {
  id?: number;
  ok: boolean;
  path?: string;
  package: Package;
  asset: Asset;
}

// Internally-tagged on the Rust side:
//   #[serde(tag = "kind", rename_all = "camelCase")]
// Wire format:
//   { kind: "added",   path, ty, value }
//   { kind: "removed", path, ty, value }
//   { kind: "changed", path, ty, old, new }
export type PropertyChange =
  | { kind: "added";   path: string; ty: string; value: PropertyValue }
  | { kind: "removed"; path: string; ty: string; value: PropertyValue }
  | { kind: "changed"; path: string; ty: string; old: PropertyValue; new: PropertyValue };

// Rust: graph_diff::NodeStatus — serde(rename_all = "lowercase")
// Values on the wire: "added", "removed", "changed", "unchanged"
export type NodeStatus = "added" | "removed" | "changed" | "unchanged";

// Rust: graph_diff::GraphDiff — serde(rename_all = "camelCase")
export interface GraphDiff {
  name: string;
  onlyInOurs: boolean;
  onlyInTheirs: boolean;
  nodeStatuses: Record<string, NodeStatus>;
}

export type AppMode =
  | { kind: "cli" }
  | { kind: "standaloneGui" }
  | { kind: "gitDriverGui"; ancestor: string; ours: string; theirs: string; path: string };

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
