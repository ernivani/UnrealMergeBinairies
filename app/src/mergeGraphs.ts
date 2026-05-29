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

// Canonicalise a node blob for semantic comparison - mirrors Rust
// graph_diff::normalize_blob. Drops cosmetic NodePosX/NodePosY (UE rewrites
// these on any edit) and whitespace so a node that only moved compares equal.
// Strip volatile, non-semantic bits so two exports of the SAME logic compare
// equal (kept in sync with Rust graph_diff::normalize_blob):
//   ExportPath (per-file package path), PinToolTip/PinFriendlyName (display),
//   32-hex GUIDs (PinId/link/member/persistent), and K2Node_<Class>_<index>.
export function normalizeBlob(blob: string): string {
  return blob
    .split(/\r?\n/)
    .map((l) =>
      l
        .trim()
        .replace(/\s*ExportPath="[^"]*"/g, "")
        .replace(/,?PinToolTip="(?:[^"\\]|\\.)*"/g, "")
        .replace(/,?PinFriendlyName=NSLOCTEXT\([^)]*\)/g, "")
        .replace(/,?PinFriendlyName="(?:[^"\\]|\\.)*"/g, "")
        .replace(/(?<![0-9A-Fa-f])[0-9A-Fa-f]{32}(?![0-9A-Fa-f])/g, "<GUID>")
        .replace(/(K2Node_[A-Za-z]+)_\d+/g, "$1"),
    )
    .filter((l) => l && !l.startsWith("NodePosX=") && !l.startsWith("NodePosY="))
    .join("\n");
}

// --- Graph-level resolution (reliable writeback) --------------------------
// Node-level stitching crashes UE's importer on real graphs (function graphs,
// mixed node sets). So the WRITE is per-graph: each graph is taken whole from
// one side. The per-node view is kept only for review.

export type GraphChange = "unchanged" | "oursOnly" | "theirsOnly" | "both";

function nodeSetNorm(text?: string): Set<string> {
  const s = new Set<string>();
  for (const [, blob] of parseNodeBlobs(text ?? "")) s.add(normalizeBlob(blob));
  return s;
}
function setsEqual(a: Set<string>, b: Set<string>): boolean {
  if (a.size !== b.size) return false;
  for (const x of a) if (!b.has(x)) return false;
  return true;
}

// Did each side change this graph (vs ancestor), comparing normalized node sets.
export function graphChange(anc?: string, ours?: string, theirs?: string): GraphChange {
  const a = nodeSetNorm(anc);
  const oursChanged = !setsEqual(nodeSetNorm(ours), a);
  const theirsChanged = !setsEqual(nodeSetNorm(theirs), a);
  if (oursChanged && theirsChanged) return "both";
  if (oursChanged) return "oursOnly";
  if (theirsChanged) return "theirsOnly";
  return "unchanged";
}

// Winning side for a graph; `sel` only consulted for "both".
export function graphWinner(change: GraphChange, sel: MergeSide | undefined): "ours" | "theirs" {
  if (change === "theirsOnly") return "theirs";
  if (change === "oursOnly" || change === "unchanged") return "ours";
  return sel === "theirs" ? "theirs" : "ours";
}

// Graphs the writeback must overwrite onto the ours base: only those where
// theirs wins (each emitted as theirs' full, internally-consistent text).
export function buildMergedGraphsByGraph(
  graphNames: string[],
  ancestor: Record<string, string>,
  ours: Record<string, string>,
  theirs: Record<string, string>,
  graphSel: Map<string, MergeSide>,
): Record<string, string> {
  const out: Record<string, string> = {};
  for (const g of graphNames) {
    const change = graphChange(ancestor[g], ours[g], theirs[g]);
    if (graphWinner(change, graphSel.get(g)) === "theirs" && theirs[g] != null) {
      out[g] = theirs[g];
    }
  }
  return out;
}

// GUIDs whose node is identical (semantically) in both ours and theirs - i.e.
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
    case "removedInTheirs":  // "ours kept the node" - pick ours
    case "addedInBoth":
      return "ours";
    case "modifiedInTheirs":
    case "addedInTheirs":
    case "removedInOurs":    // "theirs kept the node" - pick theirs
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

// --- Canonicalized per-node merge -----------------------------------------
// Stitching nodes from different versions naively crashes UE's importer: each
// export assigns different pin IDs / object-name indices, so a node taken from
// "ours" links to a pin ID that doesn't exist on a neighbour taken from
// "theirs". We fix this by giving every merged node a canonical name (N_<guid>)
// and rewriting every LinkedTo reference to use the target's canonical name +
// the target's chosen-side pin ID. Links to non-included nodes are dropped.

interface NodeMeta {
  name: string;
  blob: string;
}
interface SideMaps {
  nameToGuid: Map<string, string>;
  pinIdToTarget: Map<string, { guid: string; key: string }>;
}

// guid -> { name, blob } for a graph's serialization text.
function parseNodesMeta(text: string): Map<string, NodeMeta> {
  const map = new Map<string, NodeMeta>();
  if (!text) return map;
  const lines = text.split(/\r?\n/);
  let inNode = false, depth = 0, buf: string[] = [], guid: string | null = null, name = "";
  for (const line of lines) {
    const t = line.trim();
    if (!inNode) {
      if (t.startsWith("Begin Object")) {
        inNode = true;
        depth = 1;
        buf = [line];
        guid = null;
        name = t.match(/Name="([^"]+)"/)?.[1] ?? "";
      }
    } else {
      buf.push(line);
      if (t.startsWith("Begin Object")) depth++;
      else if (t.startsWith("End Object")) {
        depth--;
        if (depth === 0) {
          if (guid) map.set(guid, { name, blob: buf.join("\n") });
          inNode = false;
          buf = [];
          guid = null;
        }
      } else if (depth === 1 && t.startsWith("NodeGuid=")) {
        guid = t.slice("NodeGuid=".length).trim();
      }
    }
  }
  return map;
}

// Pins of a node: [{ pinId, key }] where key = pinName|direction.
function pinsOf(blob: string): Array<{ pinId: string; key: string }> {
  const pins: Array<{ pinId: string; key: string }> = [];
  for (const line of blob.split(/\r?\n/)) {
    const t = line.trim();
    if (!t.startsWith("CustomProperties Pin (")) continue;
    const id = t.match(/PinId=([0-9A-Fa-f]{32})/);
    const nm = t.match(/PinName="([^"]+)"/);
    const dir = t.match(/Direction="([^"]+)"/);
    if (id && nm) pins.push({ pinId: id[1], key: `${nm[1]}|${dir ? dir[1] : "in"}` });
  }
  return pins;
}

function buildSideMaps(nodes: Map<string, NodeMeta>): SideMaps {
  const nameToGuid = new Map<string, string>();
  const pinIdToTarget = new Map<string, { guid: string; key: string }>();
  for (const [guid, n] of nodes) {
    if (n.name) nameToGuid.set(n.name, guid);
    for (const p of pinsOf(n.blob)) pinIdToTarget.set(p.pinId, { guid, key: p.key });
  }
  return { nameToGuid, pinIdToTarget };
}

const canonName = (guid: string): string => `N_${guid}`;

// Rewrite one chosen node's blob: canonical own name, drop ExportPath, and
// remap every LinkedTo entry to the target's canonical name + chosen-side pin.
function rewriteBlob(
  guid: string,
  blob: string,
  origName: string,
  sideMap: SideMaps,
  included: Set<string>,
  outPinByGuid: Map<string, Map<string, string>>,
): string {
  const rewritten = blob
    .split(/\r?\n/)
    .map((line) => {
      let s = line.replace(/\s*ExportPath="[^"]*"/g, "");
      s = s.replace(/LinkedTo=\(([^)]*)\)/g, (_m, inner: string) => {
        const kept: string[] = [];
        for (const raw of inner.split(",")) {
          const e = raw.trim();
          if (!e) continue;
          const sp = e.lastIndexOf(" ");
          if (sp < 0) continue;
          const tName = e.slice(0, sp);
          const tPin = e.slice(sp + 1);
          const tGuid = sideMap.nameToGuid.get(tName);
          const tInfo = sideMap.pinIdToTarget.get(tPin);
          if (!tGuid || !tInfo || !included.has(tGuid)) continue; // drop dangling
          const newPin = outPinByGuid.get(tGuid)?.get(tInfo.key);
          if (!newPin) continue;
          kept.push(`${canonName(tGuid)} ${newPin}`);
        }
        return "LinkedTo=(" + kept.map((k) => k + ",").join("") + ")";
      });
      return s;
    })
    .join("\n");
  // Canonical own name (only on the Begin Object line).
  return origName ? rewritten.replace(`Name="${origName}"`, `Name="${canonName(guid)}"`) : rewritten;
}

// Build the merged text per graph from per-node selections, canonicalized so
// the result is internally consistent and UE can import it without crashing.
export function buildMergedGraphs(
  threeWayDiffs: ThreeWayGraphDiff[],
  ancestorGraphs: Record<string, string>,
  oursGraphs: Record<string, string>,
  theirsGraphs: Record<string, string>,
  selections: Map<string /* graphName */, Map<string /* guid */, MergeSide>>,
): Record<string, string> {
  void ancestorGraphs; // unchanged/common nodes are taken from ours
  const out: Record<string, string> = {};

  for (const diff of threeWayDiffs) {
    const ours = parseNodesMeta(oursGraphs[diff.name] ?? "");
    const theirs = parseNodesMeta(theirsGraphs[diff.name] ?? "");
    const nodesBy: Record<"ours" | "theirs", Map<string, NodeMeta>> = { ours, theirs };
    const sideMaps = { ours: buildSideMaps(ours), theirs: buildSideMaps(theirs) };
    const graphSel = selections.get(diff.name) ?? new Map<string, MergeSide>();

    // Decide which node comes from which side.
    const chosen = new Map<string, "ours" | "theirs">();
    for (const [guid, status] of Object.entries(diff.nodeStatuses)) {
      // Unchanged/common nodes are kept (from ours); removedInBoth is dropped;
      // everything else follows the user pick or the per-status default.
      let picked: MergeSide | null;
      const sel = graphSel.get(guid);
      if (sel !== undefined) picked = sel;
      else if (status === "unchanged") picked = "ours";
      else picked = defaultSide(status);
      if (picked === null || picked === "skip") continue;
      // Fall back to whichever side actually has the node.
      const side: "ours" | "theirs" =
        picked === "theirs" ? (theirs.has(guid) ? "theirs" : "ours") : ours.has(guid) ? "ours" : "theirs";
      if (nodesBy[side].has(guid)) chosen.set(guid, side);
    }

    // Only emit graphs whose merged result actually differs from ours' base.
    // The writeback rewrites every emitted graph (clear + re-import), which is
    // risky for function graphs (entry/result nodes) and pointless when the
    // result equals ours. If nothing came from theirs and no ours node was
    // dropped, the ours base is already correct - leave the graph untouched.
    const oursGuids = new Set(ours.keys());
    let differs = false;
    for (const [guid, side] of chosen) {
      if (side === "theirs" || !oursGuids.has(guid)) { differs = true; break; }
    }
    if (!differs) {
      for (const g of oursGuids) if (!chosen.has(g)) { differs = true; break; }
    }
    if (!differs) continue;

    const included = new Set(chosen.keys());
    // Output pin map per node (keys -> chosen-side pin ids).
    const outPinByGuid = new Map<string, Map<string, string>>();
    for (const [guid, side] of chosen) {
      const m = new Map<string, string>();
      for (const p of pinsOf(nodesBy[side].get(guid)!.blob)) m.set(p.key, p.pinId);
      outPinByGuid.set(guid, m);
    }

    const blobs: string[] = [];
    for (const [guid, side] of chosen) {
      const meta = nodesBy[side].get(guid)!;
      blobs.push(rewriteBlob(guid, meta.blob, meta.name, sideMaps[side], included, outPinByGuid));
    }
    out[diff.name] = blobs.join("\n") + (blobs.length ? "\n" : "");
  }

  return out;
}
