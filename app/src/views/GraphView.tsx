import { useEffect, useMemo, useRef, useState } from "react";
import type {
  AssetSnapshot,
  GraphDiff,
  MergeSide,
  ThreeWayGraphDiff,
  ThreeWayNodeStatus,
} from "../types";
import { isConflictStatus } from "../types";
import { buildMergedGraphs, commonGuids, defaultSide } from "../mergeGraphs";
import GraphPane from "./GraphPane";
import styles from "./GraphView.module.css";

interface Props {
  ours: AssetSnapshot;
  theirs: AssetSnapshot;
  graphDiffs: GraphDiff[];
  /** Optional ancestor. When present, GraphView enters three-way mode. */
  ancestor?: AssetSnapshot;
  threeWayDiffs?: ThreeWayGraphDiff[];
  /** Per-graph per-GUID selection state, owned by Diff.tsx. */
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

  const [activeGraph, setActiveGraph] = useState<string>(() => allGraphNames[0] ?? "");
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

  const threeWayMode = ancestor != null && threeWayDiffs != null && onSelectionChange != null;

  const oursText = ours.asset.graphs?.[activeGraph];
  const theirsText = theirs.asset.graphs?.[activeGraph];

  const common = useMemo(
    () => (threeWayMode ? commonGuids(oursText, theirsText) : new Set<string>()),
    [threeWayMode, oursText, theirsText],
  );

  // Live merged "result" graph, rebuilt whenever a node pick changes.
  const resultText = useMemo(() => {
    if (!threeWayMode || !threeWayDiffs) return undefined;
    const merged = buildMergedGraphs(
      threeWayDiffs,
      ancestor?.asset.graphs ?? {},
      ours.asset.graphs ?? {},
      theirs.asset.graphs ?? {},
      selections ?? new Map(),
    );
    return merged[activeGraph];
  }, [threeWayMode, threeWayDiffs, ancestor, ours, theirs, selections, activeGraph]);

  // Real conflicts = conflict status and not common/agreed.
  const conflictGuids = useMemo(() => {
    if (!activeThreeWayDiff) return [] as string[];
    return Object.entries(activeThreeWayDiff.nodeStatuses)
      .filter(([guid, s]) => isConflictStatus(s as ThreeWayNodeStatus) && !common.has(guid))
      .map(([guid]) => guid);
  }, [activeThreeWayDiff, common]);

  const onlyInOurs = oursText != null && theirsText == null;
  const onlyInTheirs = oursText == null && theirsText != null;

  const resultWrapRef = useRef<HTMLDivElement>(null);

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
        {onlyInOurs && <span className={`${styles.badge} ${styles.badgeOurs}`}>only in Ours</span>}
        {onlyInTheirs && <span className={`${styles.badge} ${styles.badgeTheirs}`}>only in Theirs</span>}
        {activeThreeWayDiff && (
          <span className={`${styles.conflictSummary} ${conflictGuids.length === 0 ? styles.noConflicts : ""}`}>
            {conflictGuids.length === 0
              ? "no conflicts"
              : `${conflictGuids.length} conflict${conflictGuids.length === 1 ? "" : "s"} (pick on the middle nodes)`}
          </span>
        )}
      </div>

      <div className={styles.split}>
        <GraphPane
          label="Ours"
          side="ours"
          graphText={oursText}
          diff={threeWayMode ? undefined : activeDiff}
          threeWayDiff={activeThreeWayDiff}
          selections={activeSelections}
          common={common}
        />

        {threeWayMode && activeThreeWayDiff && (
          <div className={styles.resultWrap} ref={resultWrapRef}>
            <GraphPane
              label="Result (merged)"
              side="result"
              graphText={resultText}
              diff={undefined}
              threeWayDiff={activeThreeWayDiff}
              selections={activeSelections}
              common={common}
            />
            <ConflictBadges
              containerRef={resultWrapRef}
              conflictGuids={conflictGuids}
              selections={activeSelections}
              statuses={activeThreeWayDiff.nodeStatuses}
              graphText={resultText}
              onPick={(guid, side) => onSelectionChange!(activeGraph, guid, side)}
            />
          </div>
        )}

        <GraphPane
          label="Theirs"
          side="theirs"
          graphText={theirsText}
          diff={threeWayMode ? undefined : activeDiff}
          threeWayDiff={activeThreeWayDiff}
          selections={activeSelections}
          common={common}
        />
      </div>
    </div>
  );
}

interface BadgesProps {
  containerRef: React.RefObject<HTMLDivElement | null>;
  conflictGuids: string[];
  selections: Map<string, MergeSide>;
  statuses: Record<string, ThreeWayNodeStatus>;
  graphText: string | undefined;
  onPick: (guid: string, side: MergeSide) => void;
}

// Floating Ours/Theirs/Skip control over each conflict node in the result pane.
function ConflictBadges({ containerRef, conflictGuids, selections, statuses, graphText, onPick }: BadgesProps) {
  const [positions, setPositions] = useState<Array<{ guid: string; top: number; left: number }>>([]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container || conflictGuids.length === 0) {
      setPositions([]);
      return;
    }
    function recompute() {
      const cont = containerRef.current;
      if (!cont) return;
      const base = cont.getBoundingClientRect();
      const next: Array<{ guid: string; top: number; left: number }> = [];
      cont.querySelectorAll("ueb-node").forEach((el) => {
        const nodeEl = el as HTMLElement & { entity?: { NodeGuid?: { toString(): string } } };
        // Canonical merged node names are N_<guid>; match on the entity guid.
        const guid = nodeEl.entity?.NodeGuid?.toString();
        if (!guid || !conflictGuids.includes(guid)) return;
        const r = el.getBoundingClientRect();
        next.push({ guid, top: r.top - base.top, left: r.left - base.left + r.width / 2 - 48 });
      });
      setPositions(next);
    }
    recompute();
    const observer = new MutationObserver(recompute);
    observer.observe(container, { childList: true, subtree: true, attributes: true });
    const interval = window.setInterval(recompute, 800);
    return () => {
      observer.disconnect();
      window.clearInterval(interval);
    };
  }, [containerRef, conflictGuids, graphText]);

  return (
    <>
      {positions.map(({ guid, top, left }) => {
        const cur = selections.get(guid) ?? defaultSide(statuses[guid]);
        return (
          <div key={guid} className={styles.badge3} style={{ top, left }}>
            <button
              className={`${styles.badge3Btn} ${cur === "ours" ? styles.badge3Ours : ""}`}
              onClick={() => onPick(guid, "ours")}
              title="Take Ours"
            >
              Ours
            </button>
            <button
              className={`${styles.badge3Btn} ${cur === "theirs" ? styles.badge3Theirs : ""}`}
              onClick={() => onPick(guid, "theirs")}
              title="Take Theirs"
            >
              Theirs
            </button>
            <button
              className={`${styles.badge3Btn} ${cur === "skip" ? styles.badge3Skip : ""}`}
              onClick={() => onPick(guid, "skip")}
              title="Skip this node"
            >
              Skip
            </button>
          </div>
        );
      })}
    </>
  );
}
