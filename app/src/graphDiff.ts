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
