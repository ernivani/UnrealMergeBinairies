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
