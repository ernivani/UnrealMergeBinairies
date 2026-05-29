import { useEffect, useMemo, useRef, useState } from "react";
import type { AssetSnapshot, GraphDiff, MergeSide, ThreeWayGraphDiff } from "../types";
import { commonGuids, parseNodeBlobs } from "../mergeGraphs";
import GraphPane from "./GraphPane";
import styles from "./GraphView.module.css";

interface Props {
  ours: AssetSnapshot;
  theirs: AssetSnapshot;
  graphDiffs: GraphDiff[];
  ancestor?: AssetSnapshot;
  threeWayDiffs?: ThreeWayGraphDiff[];
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
    if (allGraphNames.length > 0 && !allGraphNames.includes(activeGraph)) setActiveGraph(allGraphNames[0]);
  }, [allGraphNames, activeGraph]);

  const activeDiff = useMemo(() => graphDiffs.find((d) => d.name === activeGraph), [graphDiffs, activeGraph]);
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

  const oursChangeGuids = useMemo(() => changeGuids(oursText, activeThreeWayDiff, common), [oursText, activeThreeWayDiff, common]);
  const theirsChangeGuids = useMemo(() => changeGuids(theirsText, activeThreeWayDiff, common), [theirsText, activeThreeWayDiff, common]);

  const onlyInOurs = oursText != null && theirsText == null;
  const onlyInTheirs = oursText == null && theirsText != null;

  const oursWrapRef = useRef<HTMLDivElement>(null);
  const theirsWrapRef = useRef<HTMLDivElement>(null);

  return (
    <div className={styles.container}>
      <div className={styles.toolbar}>
        <span>Graph:</span>
        <select className={styles.graphSelect} value={activeGraph} onChange={(e) => setActiveGraph(e.target.value)}>
          {allGraphNames.map((name) => (
            <option key={name} value={name}>{name}</option>
          ))}
        </select>
        {onlyInOurs && <span className={`${styles.badge} ${styles.badgeOurs}`}>only in Ours</span>}
        {onlyInTheirs && <span className={`${styles.badge} ${styles.badgeTheirs}`}>only in Theirs</span>}
        {threeWayMode && (
          <span className={styles.note}>
            {oursChangeGuids.length + theirsChangeGuids.length} changed node
            {oursChangeGuids.length + theirsChangeGuids.length === 1 ? "" : "s"} - Keep or Drop each
          </span>
        )}
      </div>

      <div className={styles.split}>
        <div className={styles.paneWrap} ref={oursWrapRef}>
          <GraphPane label="Ours" side="ours" graphText={oursText} diff={threeWayMode ? undefined : activeDiff} threeWayDiff={activeThreeWayDiff} selections={activeSelections} common={common} />
          {threeWayMode && (
            <NodeBadges containerRef={oursWrapRef} guids={oursChangeGuids} keepSide="ours" selections={activeSelections} graphText={oursText} onPick={(g, s) => onSelectionChange!(activeGraph, g, s)} />
          )}
        </div>
        <div className={styles.paneWrap} ref={theirsWrapRef}>
          <GraphPane label="Theirs" side="theirs" graphText={theirsText} diff={threeWayMode ? undefined : activeDiff} threeWayDiff={activeThreeWayDiff} selections={activeSelections} common={common} />
          {threeWayMode && (
            <NodeBadges containerRef={theirsWrapRef} guids={theirsChangeGuids} keepSide="theirs" selections={activeSelections} graphText={theirsText} onPick={(g, s) => onSelectionChange!(activeGraph, g, s)} />
          )}
        </div>
      </div>
    </div>
  );
}

function changeGuids(text: string | undefined, diff: ThreeWayGraphDiff | undefined, common: Set<string>): string[] {
  if (!text || !diff) return [];
  const present = new Set(parseNodeBlobs(text).keys());
  return Object.entries(diff.nodeStatuses)
    .filter(([guid, s]) => present.has(guid) && s !== "unchanged" && s !== "removedInBoth" && !common.has(guid))
    .map(([guid]) => guid);
}

interface BadgesProps {
  containerRef: React.RefObject<HTMLDivElement | null>;
  guids: string[];
  keepSide: "ours" | "theirs";
  selections: Map<string, MergeSide>;
  graphText: string | undefined;
  onPick: (guid: string, side: MergeSide) => void;
}

// Keep / Drop control on each changed node in a side pane.
function NodeBadges({ containerRef, guids, keepSide, selections, graphText, onPick }: BadgesProps) {
  const [positions, setPositions] = useState<Array<{ guid: string; top: number; left: number }>>([]);
  useEffect(() => {
    const container = containerRef.current;
    if (!container || guids.length === 0) { setPositions([]); return; }
    function recompute() {
      const cont = containerRef.current;
      if (!cont) return;
      const base = cont.getBoundingClientRect();
      const next: Array<{ guid: string; top: number; left: number }> = [];
      cont.querySelectorAll("ueb-node").forEach((el) => {
        const nodeEl = el as HTMLElement & { entity?: { NodeGuid?: { toString(): string } } };
        const guid = nodeEl.entity?.NodeGuid?.toString();
        if (!guid || !guids.includes(guid)) return;
        const r = el.getBoundingClientRect();
        const cx = r.left + r.width / 2, cy = r.top + r.height / 2;
        if (cx < base.left || cx > base.right || cy < base.top || cy > base.bottom) return;
        next.push({ guid, top: cy - base.top - 11, left: cx - base.left - 32 });
      });
      setPositions(next);
    }
    recompute();
    const observer = new MutationObserver(recompute);
    observer.observe(container, { childList: true, subtree: true, attributes: true });
    const interval = window.setInterval(recompute, 400);
    return () => { observer.disconnect(); window.clearInterval(interval); };
  }, [containerRef, guids, graphText]);

  return (
    <>
      {positions.map(({ guid, top, left }) => {
        const cur = selections.get(guid) ?? "skip";
        return (
          <div key={guid} className={styles.keepDrop} style={{ top, left }}>
            <button className={`${styles.kdBtn} ${cur === keepSide ? styles.kdKeepOn : ""}`} onClick={() => onPick(guid, keepSide)} title="Keep this node">✓ Keep</button>
            <button className={`${styles.kdBtn} ${cur === "skip" ? styles.kdDropOn : ""}`} onClick={() => onPick(guid, "skip")} title="Don't keep this node">✕</button>
          </div>
        );
      })}
    </>
  );
}
