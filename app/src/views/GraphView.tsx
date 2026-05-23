import { useEffect, useMemo, useState } from "react";
import type {
  AssetSnapshot,
  GraphDiff,
  MergeSide,
  ThreeWayGraphDiff,
  ThreeWayNodeStatus,
} from "../types";
import { isConflictStatus } from "../types";
import { defaultSide } from "../mergeGraphs";
import GraphPane from "./GraphPane";
import styles from "./GraphView.module.css";

interface Props {
  ours: AssetSnapshot;
  theirs: AssetSnapshot;
  graphDiffs: GraphDiff[];
  /** Optional ancestor — when present, GraphView enters three-way mode. */
  ancestor?: AssetSnapshot;
  threeWayDiffs?: ThreeWayGraphDiff[];
  /** Per-graph per-GUID selection state, owned by Diff.tsx and passed through. */
  selections?: Map<string, Map<string, MergeSide>>;
  onSelectionChange?: (graphName: string, guid: string, side: MergeSide) => void;
}

export default function GraphView({
  ours,
  theirs,
  graphDiffs,
  ancestor,
  threeWayDiffs,
  selections,
  onSelectionChange,
}: Props) {
  const allGraphNames = useMemo(() => {
    const names = new Set<string>([
      ...Object.keys(ours.asset.graphs ?? {}),
      ...Object.keys(theirs.asset.graphs ?? {}),
      ...Object.keys(ancestor?.asset.graphs ?? {}),
    ]);
    const sorted = Array.from(names).sort();
    const eventIdx = sorted.indexOf("EventGraph");
    if (eventIdx > 0) {
      sorted.splice(eventIdx, 1);
      sorted.unshift("EventGraph");
    }
    return sorted;
  }, [ours.asset.graphs, theirs.asset.graphs, ancestor?.asset.graphs]);

  const [activeGraph, setActiveGraph] = useState<string>(
    () => allGraphNames[0] ?? "",
  );
  useEffect(() => {
    if (allGraphNames.length > 0 && !allGraphNames.includes(activeGraph)) {
      setActiveGraph(allGraphNames[0]);
    }
  }, [allGraphNames, activeGraph]);

  const activeDiff = useMemo(
    () => graphDiffs.find((d) => d.name === activeGraph),
    [graphDiffs, activeGraph],
  );
  const activeThreeWayDiff = useMemo(
    () => threeWayDiffs?.find((d) => d.name === activeGraph),
    [threeWayDiffs, activeGraph],
  );
  const activeSelections = useMemo(
    () => selections?.get(activeGraph) ?? new Map<string, MergeSide>(),
    [selections, activeGraph],
  );

  const oursText = ours.asset.graphs?.[activeGraph];
  const theirsText = theirs.asset.graphs?.[activeGraph];

  const onlyInOurs =
    activeThreeWayDiff?.onlyInOurs ??
    activeDiff?.onlyInOurs ??
    (oursText != null && theirsText == null);
  const onlyInTheirs =
    activeThreeWayDiff?.onlyInTheirs ??
    activeDiff?.onlyInTheirs ??
    (oursText == null && theirsText != null);

  // Conflict summary for the toolbar (only meaningful in 3-way mode).
  const conflictGuids = useMemo(() => {
    if (!activeThreeWayDiff) return [] as string[];
    return Object.entries(activeThreeWayDiff.nodeStatuses)
      .filter(([, s]) => isConflictStatus(s as ThreeWayNodeStatus))
      .map(([guid]) => guid);
  }, [activeThreeWayDiff]);

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
          <span className={`${styles.badge} ${styles.badgeOurs}`}>only in Ours</span>
        )}
        {onlyInTheirs && (
          <span className={`${styles.badge} ${styles.badgeTheirs}`}>only in Theirs</span>
        )}
        {activeThreeWayDiff && (
          <span className={`${styles.conflictSummary} ${conflictGuids.length === 0 ? styles.noConflicts : ""}`}>
            {conflictGuids.length === 0
              ? "no conflicts"
              : `${conflictGuids.length} conflict${conflictGuids.length === 1 ? "" : "s"}`}
          </span>
        )}
      </div>

      <div className={styles.split} style={{ position: "relative" }}>
        <GraphPane
          label="Ours"
          side="ours"
          graphText={oursText}
          diff={activeThreeWayDiff ? undefined : activeDiff}
          threeWayDiff={activeThreeWayDiff}
          selections={activeSelections}
        />
        <GraphPane
          label="Theirs"
          side="theirs"
          graphText={theirsText}
          diff={activeThreeWayDiff ? undefined : activeDiff}
          threeWayDiff={activeThreeWayDiff}
          selections={activeSelections}
        />
        {activeThreeWayDiff && onSelectionChange && (
          <ConflictPickers
            diff={activeThreeWayDiff}
            selections={activeSelections}
            onPick={(guid, side) => onSelectionChange(activeGraph, guid, side)}
          />
        )}
      </div>
    </div>
  );
}

interface PickersProps {
  diff: ThreeWayGraphDiff;
  selections: Map<string, MergeSide>;
  onPick: (guid: string, side: MergeSide) => void;
}

// Renders a small floating picker per conflicting node. Position-tracking
// uses a MutationObserver to find <ueb-node> elements and align to them.
function ConflictPickers({ diff, selections, onPick }: PickersProps) {
  const [positions, setPositions] = useState<Array<{ guid: string; top: number; left: number }>>([]);

  useEffect(() => {
    const conflicts = Object.entries(diff.nodeStatuses)
      .filter(([, s]) => isConflictStatus(s as ThreeWayNodeStatus))
      .map(([guid]) => guid);

    if (conflicts.length === 0) {
      setPositions([]);
      return;
    }

    // The pickers position over the OURS pane (first pane). Find its DOM.
    const container = document.querySelector(`.${styles.split}`);
    if (!container) return;
    const oursPane = container.children[0] as HTMLElement | undefined;
    if (!oursPane) return;

    function recompute() {
      const next: Array<{ guid: string; top: number; left: number }> = [];
      const containerRect = oursPane!.getBoundingClientRect();
      const nodeEls = oursPane!.querySelectorAll("ueb-node");
      nodeEls.forEach((el) => {
        const nodeEl = el as HTMLElement & { entity?: { NodeGuid?: { toString(): string } } };
        const guid = nodeEl.entity?.NodeGuid?.toString();
        if (!guid || !conflicts.includes(guid)) return;
        const r = el.getBoundingClientRect();
        next.push({
          guid,
          top: r.top - containerRect.top,
          left: r.left - containerRect.left + r.width / 2 - 40,
        });
      });
      setPositions(next);
    }

    recompute();
    const observer = new MutationObserver(recompute);
    observer.observe(oursPane, { childList: true, subtree: true, attributes: true });
    const interval = window.setInterval(recompute, 1000); // catch scroll/zoom changes

    return () => {
      observer.disconnect();
      window.clearInterval(interval);
    };
  }, [diff]);

  return (
    <>
      {positions.map(({ guid, top, left }) => {
        const status = diff.nodeStatuses[guid];
        const cur = selections.get(guid) ?? defaultSide(status);
        return (
          <div
            key={guid}
            className={styles.conflictPicker}
            style={{ top, left }}
          >
            <button
              className={`${styles.conflictPickerBtn} ${cur === "ours" ? styles.conflictPickerBtnActive : ""}`}
              onClick={() => onPick(guid, "ours")}
              title="Take Ours"
            >
              O
            </button>
            <button
              className={`${styles.conflictPickerBtn} ${cur === "theirs" ? styles.conflictPickerBtnActive : ""}`}
              onClick={() => onPick(guid, "theirs")}
              title="Take Theirs"
            >
              T
            </button>
            <button
              className={`${styles.conflictPickerBtn} ${cur === "skip" ? styles.conflictPickerBtnActive : ""}`}
              onClick={() => onPick(guid, "skip")}
              title="Skip (omit this node from the merge)"
            >
              —
            </button>
          </div>
        );
      })}
    </>
  );
}
