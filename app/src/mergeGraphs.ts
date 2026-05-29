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

// Canonicalise a node blob for semantic comparison — mirrors Rust
// graph_diff::normalize_blob. Drops cosmetic NodePosX/NodePosY (UE rewrites
// these on any edit) and whitespace so a node that only moved compares equal.
export function normalizeBlob(blob: string): string {
  return blob
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith("NodePosX=") && !l.startsWith("NodePosY="))
    .join("\n");
}

// GUIDs whose node is identical (semantically) in both ours and theirs — i.e.
// "agreed / common" nodes. These are dimmed in the UI so real differences pop.
export function commonGuids(oursText?: string, theirsText?: string): Set<string> {
  const o = parseNodeBlobs(oursText ?? "");
  const t = parseNodeBlobs(theirsText ?? "");
  const set = new Set<string>();
  for (const [guid, ob] of o) {
    const tb = t.get(guid);
    if (tb && normalizeBlob(ob) === normalizeBlob(tb)) set.add(guid);
  }
  return set;
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
