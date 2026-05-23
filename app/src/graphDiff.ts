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

    // Remove any stale diff classes from a previous overlay pass.
    nodeEl.classList.remove("uem-diff-added", "uem-diff-removed", "uem-diff-changed");

    const status: NodeStatus | undefined = diff.nodeStatuses[guid];
    if (!status || status === "unchanged") return;

    // "added" nodes only exist in theirs — no class applied on ours side (and vice versa for "removed").
    // The ueblueprint renderer may still produce a DOM node for an "added" GUID in the ours pane
    // if the graph text was shared; leaving it unstyled is intentional.

    if (status === "added" && side === "theirs") {
      nodeEl.classList.add("uem-diff-added");
    } else if (status === "removed" && side === "ours") {
      nodeEl.classList.add("uem-diff-removed");
    } else if (status === "changed") {
      nodeEl.classList.add("uem-diff-changed");
    }
  });
}

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
