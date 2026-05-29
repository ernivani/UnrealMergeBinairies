import { useEffect, useMemo, useRef, useState } from "react";
import type { AssetSnapshot, GraphDiff, MergeSide, ThreeWayGraphDiff } from "../types";
import { buildMergedGraphs, commonGuids, parseNodeBlobs } from "../mergeGraphs";
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

  // Per-side change sets: nodes present on that side that aren't unchanged/common.
  const oursChangeGuids = useMemo(() => changeGuids(oursText, activeThreeWayDiff, common), [oursText, activeThreeWayDiff, common]);
  const theirsChangeGuids = useMemo(() => changeGuids(theirsText, activeThreeWayDiff, common), [theirsText, activeThreeWayDiff, common]);

  const conflictCount = useMemo(() => {
    if (!activeThreeWayDiff) return 0;
    return Object.values(activeThreeWayDiff.nodeStatuses).filter(
      (s) => s === "modifiedInBoth" || s === "addedInBothConflict" || s === "modifyDeleteConflict",
    ).length;
  }, [activeThreeWayDiff]);

  const onlyInOurs = oursText != null && theirsText == null;
  const onlyInTheirs = oursText == null && theirsText != null;

  const splitRef = useRef<HTMLDivElement>(null);
  const oursWrapRef = useRef<HTMLDivElement>(null);
  const theirsWrapRef = useRef<HTMLDivElement>(null);

  useViewportSync(splitRef, [oursText, theirsText, resultText, activeGraph]);

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
        {threeWayMode && (
          <span className={styles.hint}>
            {oursChangeGuids.length + theirsChangeGuids.length} changed node
            {oursChangeGuids.length + theirsChangeGuids.length === 1 ? "" : "s"}
            {conflictCount > 0 ? ` (${conflictCount} conflict${conflictCount === 1 ? "" : "s"})` : ""}
            {" · Keep/Drop on each side"}
          </span>
        )}
      </div>

      <div className={styles.split} ref={splitRef}>
        <div className={styles.paneWrap} ref={oursWrapRef}>
          <GraphPane
            label="Ours"
            side="ours"
            graphText={oursText}
            diff={threeWayMode ? undefined : activeDiff}
            threeWayDiff={activeThreeWayDiff}
            selections={activeSelections}
            common={common}
          />
          {threeWayMode && (
            <NodeBadges
              containerRef={oursWrapRef}
              guids={oursChangeGuids}
              keepSide="ours"
              selections={activeSelections}
              graphText={oursText}
              onPick={(guid, side) => onSelectionChange!(activeGraph, guid, side)}
            />
          )}
        </div>

        {threeWayMode && (
          <div className={styles.resultWrap}>
            <GraphPane
              label="Result (merged)"
              side="result"
              graphText={resultText}
              diff={undefined}
              threeWayDiff={activeThreeWayDiff}
              selections={activeSelections}
              common={common}
            />
          </div>
        )}

        <div className={styles.paneWrap} ref={theirsWrapRef}>
          <GraphPane
            label="Theirs"
            side="theirs"
            graphText={theirsText}
            diff={threeWayMode ? undefined : activeDiff}
            threeWayDiff={activeThreeWayDiff}
            selections={activeSelections}
            common={common}
          />
          {threeWayMode && (
            <NodeBadges
              containerRef={theirsWrapRef}
              guids={theirsChangeGuids}
              keepSide="theirs"
              selections={activeSelections}
              graphText={theirsText}
              onPick={(guid, side) => onSelectionChange!(activeGraph, guid, side)}
            />
          )}
        </div>
      </div>
    </div>
  );
}

// Guids present on a side's graph that are changed (not unchanged, not common).
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

// Keep / Drop control on each changed node in a side pane (JetBrains-style
// accept / reject). Keep includes this side's version; Drop excludes the node.
function NodeBadges({ containerRef, guids, keepSide, selections, graphText, onPick }: BadgesProps) {
  const [positions, setPositions] = useState<Array<{ guid: string; top: number; left: number }>>([]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container || guids.length === 0) {
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
        const guid = nodeEl.entity?.NodeGuid?.toString();
        if (!guid || !guids.includes(guid)) return;
        const r = el.getBoundingClientRect();
        next.push({ guid, top: r.top - base.top, left: r.left - base.left + r.width / 2 - 32 });
      });
      setPositions(next);
    }
    recompute();
    const observer = new MutationObserver(recompute);
    observer.observe(container, { childList: true, subtree: true, attributes: true });
    const interval = window.setInterval(recompute, 400);
    return () => {
      observer.disconnect();
      window.clearInterval(interval);
    };
  }, [containerRef, guids, graphText]);

  return (
    <>
      {positions.map(({ guid, top, left }) => {
        const cur = selections.get(guid) ?? "skip";
        const kept = cur === keepSide;
        return (
          <div key={guid} className={styles.keepDrop} style={{ top, left }}>
            <button
              className={`${styles.kdBtn} ${kept ? styles.kdKeepOn : ""}`}
              onClick={() => onPick(guid, keepSide)}
              title="Keep this node"
            >
              ✓ Keep
            </button>
            <button
              className={`${styles.kdBtn} ${cur === "skip" ? styles.kdDropOn : ""}`}
              onClick={() => onPick(guid, "skip")}
              title="Don't keep this node"
            >
              ✕
            </button>
          </div>
        );
      })}
    </>
  );
}

// Keep the three blueprint viewports aligned: when the user pans/zooms one,
// mirror its scroll + zoom to the others. Uses ueblueprint's element API
// (getScroll/setScroll/zoom) defensively — no-ops if unavailable.
function useViewportSync(splitRef: React.RefObject<HTMLDivElement | null>, deps: unknown[]) {
  useEffect(() => {
    const root = splitRef.current;
    if (!root) return;
    let raf = 0;
    let last: { x: unknown; y: unknown; z: unknown } | null = null;
    let applying = false;

    const blueprints = () => Array.from(root.querySelectorAll("ueb-blueprint")) as any[];

    function read(b: any): { x: unknown; y: unknown; z: unknown } | null {
      try {
        const s = typeof b.getScroll === "function" ? b.getScroll() : [b.scrollX, b.scrollY];
        return { x: s?.[0] ?? s?.x ?? b.scrollX, y: s?.[1] ?? s?.y ?? b.scrollY, z: b.zoom };
      } catch {
        return null;
      }
    }
    function apply(b: any, v: { x: any; y: any; z: unknown }) {
      try {
        if (typeof b.setScroll === "function") b.setScroll([v.x, v.y]);
        else { b.scrollX = v.x; b.scrollY = v.y; }
        if (b.zoom !== v.z && v.z != null) b.zoom = v.z;
      } catch {
        /* ignore */
      }
    }

    function tick() {
      const bps = blueprints();
      if (bps.length >= 2 && !applying) {
        // Find the pane that changed since last tick.
        for (const b of bps) {
          const cur = read(b);
          if (!cur) continue;
          if (last && (cur.x !== last.x || cur.y !== last.y || cur.z !== last.z)) {
            applying = true;
            for (const o of bps) if (o !== b) apply(o, cur as any);
            applying = false;
            last = cur;
            break;
          }
          if (!last) last = cur;
        }
      }
      raf = window.setTimeout(tick, 120) as unknown as number;
    }
    raf = window.setTimeout(tick, 500) as unknown as number;
    return () => window.clearTimeout(raf);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
}
