import { useEffect, useMemo, useState } from "react";
import type {
  AssetSnapshot,
  GraphDiff,
  MergeSide,
  ThreeWayGraphDiff,
  ThreeWayNodeStatus,
} from "../types";
import { isConflictStatus } from "../types";
import GraphPane from "./GraphPane";
import ResultPanel from "./ResultPanel";
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
  // The row the user clicked in the Result panel — flashed in both side panes.
  const [selectedGuid, setSelectedGuid] = useState<string | undefined>(undefined);

  useEffect(() => {
    if (allGraphNames.length > 0 && !allGraphNames.includes(activeGraph)) {
      setActiveGraph(allGraphNames[0]);
    }
  }, [allGraphNames, activeGraph]);

  // Clear the flashed node when switching graphs.
  useEffect(() => {
    setSelectedGuid(undefined);
  }, [activeGraph]);

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
  const ancestorText = ancestor?.asset.graphs?.[activeGraph];

  const onlyInOurs =
    activeThreeWayDiff?.onlyInOurs ??
    activeDiff?.onlyInOurs ??
    (oursText != null && theirsText == null);
  const onlyInTheirs =
    activeThreeWayDiff?.onlyInTheirs ??
    activeDiff?.onlyInTheirs ??
    (oursText == null && theirsText != null);

  const conflictCount = useMemo(() => {
    if (!activeThreeWayDiff) return 0;
    return Object.values(activeThreeWayDiff.nodeStatuses).filter((s) =>
      isConflictStatus(s as ThreeWayNodeStatus),
    ).length;
  }, [activeThreeWayDiff]);

  const threeWayMode = activeThreeWayDiff != null && onSelectionChange != null;

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
          <span className={`${styles.conflictSummary} ${conflictCount === 0 ? styles.noConflicts : ""}`}>
            {conflictCount === 0
              ? "no conflicts"
              : `${conflictCount} conflict${conflictCount === 1 ? "" : "s"}`}
          </span>
        )}
      </div>

      <div className={styles.split}>
        <GraphPane
          label="Ours"
          side="ours"
          graphText={oursText}
          diff={activeThreeWayDiff ? undefined : activeDiff}
          threeWayDiff={activeThreeWayDiff}
          selections={activeSelections}
          selectedGuid={selectedGuid}
        />
        {threeWayMode && activeThreeWayDiff && (
          <ResultPanel
            diff={activeThreeWayDiff}
            oursText={oursText}
            theirsText={theirsText}
            ancestorText={ancestorText}
            selections={activeSelections}
            onSelect={(guid, side) => onSelectionChange!(activeGraph, guid, side)}
            selectedGuid={selectedGuid}
            onRowClick={(guid) =>
              setSelectedGuid((prev) => (prev === guid ? undefined : guid))
            }
          />
        )}
        <GraphPane
          label="Theirs"
          side="theirs"
          graphText={theirsText}
          diff={activeThreeWayDiff ? undefined : activeDiff}
          threeWayDiff={activeThreeWayDiff}
          selections={activeSelections}
          selectedGuid={selectedGuid}
        />
      </div>
    </div>
  );
}
