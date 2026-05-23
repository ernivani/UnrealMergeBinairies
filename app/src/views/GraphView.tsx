import { useEffect, useMemo, useState } from "react";
import type { AssetSnapshot, GraphDiff } from "../types";
import GraphPane from "./GraphPane";
import styles from "./GraphView.module.css";

interface Props {
  ours: AssetSnapshot;
  theirs: AssetSnapshot;
  graphDiffs: GraphDiff[];
}

export default function GraphView({ ours, theirs, graphDiffs }: Props) {
  const allGraphNames = useMemo(() => {
    const names = new Set<string>([
      ...Object.keys(ours.asset.graphs ?? {}),
      ...Object.keys(theirs.asset.graphs ?? {}),
    ]);
    const sorted = Array.from(names).sort();
    // Put EventGraph first if present
    const eventIdx = sorted.indexOf("EventGraph");
    if (eventIdx > 0) {
      sorted.splice(eventIdx, 1);
      sorted.unshift("EventGraph");
    }
    return sorted;
  }, [ours.asset.graphs, theirs.asset.graphs]);

  const [activeGraph, setActiveGraph] = useState<string>(
    () => allGraphNames[0] ?? "",
  );

  // Reset active graph if the graph list changes (e.g. after data reload).
  // -1 = not present, 0 = already first — both excluded by > 0 in the memo.
  useEffect(() => {
    if (allGraphNames.length > 0 && !allGraphNames.includes(activeGraph)) {
      setActiveGraph(allGraphNames[0]);
    }
  }, [allGraphNames, activeGraph]);

  const activeDiff = useMemo(
    () => graphDiffs.find((d) => d.name === activeGraph),
    [graphDiffs, activeGraph],
  );

  const oursText = ours.asset.graphs?.[activeGraph];
  const theirsText = theirs.asset.graphs?.[activeGraph];

  const onlyInOurs = activeDiff?.onlyInOurs ?? (oursText != null && theirsText == null);
  const onlyInTheirs = activeDiff?.onlyInTheirs ?? (oursText == null && theirsText != null);

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
          <span className={`${styles.badge} ${styles.badgeOurs}`}>
            only in Ours
          </span>
        )}
        {onlyInTheirs && (
          <span className={`${styles.badge} ${styles.badgeTheirs}`}>
            only in Theirs
          </span>
        )}
      </div>
      <div className={styles.split}>
        <GraphPane
          label="Ours"
          side="ours"
          graphText={oursText}
          diff={activeDiff}
        />
        <GraphPane
          label="Theirs"
          side="theirs"
          graphText={theirsText}
          diff={activeDiff}
        />
      </div>
    </div>
  );
}
