import { useEffect, useMemo, useState } from "react";
import type { AssetSnapshot, GraphDiff, MergeSide, ThreeWayGraphDiff } from "../types";
import { commonGuids, graphChange, graphWinner } from "../mergeGraphs";
import GraphPane from "./GraphPane";
import styles from "./GraphView.module.css";

interface Props {
  ours: AssetSnapshot;
  theirs: AssetSnapshot;
  graphDiffs: GraphDiff[];
  /** Optional ancestor — when present, GraphView enters three-way mode. */
  ancestor?: AssetSnapshot;
  threeWayDiffs?: ThreeWayGraphDiff[];
  /** Per-GRAPH side selection (only meaningful for graphs changed on both sides). */
  selections?: Map<string, MergeSide>;
  onSelectionChange?: (graphName: string, side: MergeSide) => void;
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

  const threeWayMode = ancestor != null && threeWayDiffs != null && onSelectionChange != null;

  const oursText = ours.asset.graphs?.[activeGraph];
  const theirsText = theirs.asset.graphs?.[activeGraph];
  const ancestorText = ancestor?.asset.graphs?.[activeGraph];

  const change = useMemo(
    () => (threeWayMode ? graphChange(ancestorText, oursText, theirsText) : "unchanged"),
    [threeWayMode, ancestorText, oursText, theirsText],
  );
  const sel = selections?.get(activeGraph);
  const winner = graphWinner(change, sel);
  const resultText = winner === "theirs" ? theirsText : oursText;

  const common = useMemo(
    () => (threeWayMode ? commonGuids(oursText, theirsText) : new Set<string>()),
    [threeWayMode, oursText, theirsText],
  );

  const onlyInOurs = oursText != null && theirsText == null;
  const onlyInTheirs = oursText == null && theirsText != null;

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

        {threeWayMode && change === "both" && onSelectionChange && (
          <span className={styles.graphConflict}>
            <span className={styles.conflictTag}>conflict — both edited this graph:</span>
            <span className={styles.seg}>
              <button
                className={`${styles.segBtn} ${winner === "ours" ? styles.segOurs : ""}`}
                onClick={() => onSelectionChange(activeGraph, "ours")}
              >
                Ours
              </button>
              <button
                className={`${styles.segBtn} ${winner === "theirs" ? styles.segTheirs : ""}`}
                onClick={() => onSelectionChange(activeGraph, "theirs")}
              >
                Theirs
              </button>
            </span>
          </span>
        )}
        {threeWayMode && change === "oursOnly" && (
          <span className={styles.changeNote}>changed in Ours → kept</span>
        )}
        {threeWayMode && change === "theirsOnly" && (
          <span className={styles.changeNote}>changed in Theirs → taken</span>
        )}
        {threeWayMode && change === "unchanged" && (
          <span className={`${styles.changeNote} ${styles.muted}`}>unchanged</span>
        )}
      </div>

      {threeWayMode && change === "both" && onSelectionChange && (
        <div className={styles.conflictBanner}>
          <span className={styles.conflictBannerText}>
            ⚠ <b>{activeGraph}</b> was edited on <b>both</b> sides. Choose which version to keep —
            the middle shows your choice:
          </span>
          <button
            className={`${styles.bannerBtn} ${winner === "ours" ? styles.bannerOurs : ""}`}
            onClick={() => onSelectionChange(activeGraph, "ours")}
          >
            Keep Ours
          </button>
          <button
            className={`${styles.bannerBtn} ${winner === "theirs" ? styles.bannerTheirs : ""}`}
            onClick={() => onSelectionChange(activeGraph, "theirs")}
          >
            Keep Theirs
          </button>
        </div>
      )}

      <div className={styles.split}>
        <GraphPane
          label="Ours"
          side="ours"
          graphText={oursText}
          diff={threeWayMode ? undefined : activeDiff}
          threeWayDiff={activeThreeWayDiff}
          common={common}
        />

        {threeWayMode && (
          <div className={`${styles.resultWrap} ${winner === "theirs" ? styles.resultTheirs : styles.resultOurs}`}>
            <GraphPane
              label={`Result — ${winner === "theirs" ? "Theirs" : "Ours"}`}
              side="result"
              graphText={resultText}
              diff={undefined}
              threeWayDiff={activeThreeWayDiff}
              common={common}
            />
          </div>
        )}

        <GraphPane
          label="Theirs"
          side="theirs"
          graphText={theirsText}
          diff={threeWayMode ? undefined : activeDiff}
          threeWayDiff={activeThreeWayDiff}
          common={common}
        />
      </div>
    </div>
  );
}
