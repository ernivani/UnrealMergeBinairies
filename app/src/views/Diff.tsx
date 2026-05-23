import { useCallback, useEffect, useMemo, useState } from "react";
import {
  applyGraphMerge,
  applyResolution,
  closeWithExit,
  diffGraphs,
  diffGraphsThreeWay,
  diffSnapshots,
  exportAsset,
} from "../ipc";
import type {
  AssetSnapshot,
  GraphDiff,
  MergeSide,
  PropertyChange,
  ThreeWayGraphDiff,
  ThreeWayNodeStatus,
} from "../types";
import { isConflictStatus } from "../types";
import { buildMergedGraphs, defaultSide } from "../mergeGraphs";
import GraphView from "./GraphView";
import PropertiesDiff from "./PropertiesDiff";
import Resolve from "./Resolve";
import styles from "./Diff.module.css";

interface Props {
  oursPath: string;
  theirsPath: string;
  destPath: string;
  /** Git's %O (merge base). When provided + asset is Blueprint, enables Take Both. */
  ancestorPath?: string;
}

type Status =
  | { kind: "loading" }
  | { kind: "error"; message: string }
  | {
      kind: "ready";
      ours: AssetSnapshot;
      theirs: AssetSnapshot;
      ancestor?: AssetSnapshot;
      changes: PropertyChange[];
      graphDiffs: GraphDiff[];
      threeWayDiffs?: ThreeWayGraphDiff[];
    };

type Tab = "graph" | "properties";

export default function Diff({ oursPath, theirsPath, destPath, ancestorPath }: Props) {
  const [status, setStatus] = useState<Status>({ kind: "loading" });
  const [resolving, setResolving] = useState(false);
  const [activeTab, setActiveTab] = useState<Tab>("graph");
  // Per-graph per-GUID selections. Initialised from defaults once threeWayDiffs arrive.
  const [selections, setSelections] = useState<Map<string, Map<string, MergeSide>>>(new Map());

  useEffect(() => {
    setActiveTab("graph");
  }, [oursPath, theirsPath]);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const [ours, theirs, ancestor] = await Promise.all([
          exportAsset(oursPath),
          exportAsset(theirsPath),
          ancestorPath ? exportAsset(ancestorPath) : Promise.resolve(undefined),
        ]);
        const [changes, graphDiffs] = await Promise.all([
          diffSnapshots(ours, theirs),
          diffGraphs(ours, theirs),
        ]);
        let threeWayDiffs: ThreeWayGraphDiff[] | undefined;
        if (ancestor && ours.asset.class === "Blueprint") {
          threeWayDiffs = await diffGraphsThreeWay(ancestor, ours, theirs);
        }
        if (!cancelled) {
          setStatus({ kind: "ready", ours, theirs, ancestor, changes, graphDiffs, threeWayDiffs });
          if (threeWayDiffs) {
            // Seed selections from defaults so we always have a valid choice.
            const seed = new Map<string, Map<string, MergeSide>>();
            for (const d of threeWayDiffs) {
              const m = new Map<string, MergeSide>();
              for (const [guid, st] of Object.entries(d.nodeStatuses)) {
                const def = defaultSide(st as ThreeWayNodeStatus);
                if (def !== null) m.set(guid, def);
              }
              seed.set(d.name, m);
            }
            setSelections(seed);
          }
        }
      } catch (e) {
        if (!cancelled) setStatus({ kind: "error", message: String(e) });
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, [oursPath, theirsPath, ancestorPath]);

  const changedPaths = useMemo(() => {
    if (status.kind !== "ready") return new Set<string>();
    const s = new Set<string>();
    for (const c of status.changes) s.add(c.path);
    return s;
  }, [status]);

  const onSelectionChange = useCallback((graphName: string, guid: string, side: MergeSide) => {
    setSelections((prev) => {
      const next = new Map(prev);
      const inner = new Map(next.get(graphName) ?? new Map<string, MergeSide>());
      inner.set(guid, side);
      next.set(graphName, inner);
      return next;
    });
  }, []);

  const conflictCount = useMemo(() => {
    if (status.kind !== "ready" || !status.threeWayDiffs) return 0;
    let n = 0;
    for (const d of status.threeWayDiffs) {
      for (const st of Object.values(d.nodeStatuses)) {
        if (isConflictStatus(st as ThreeWayNodeStatus)) n += 1;
      }
    }
    return n;
  }, [status]);

  async function resolve(kind: "ours" | "theirs" | "abort" | "both") {
    setResolving(true);
    try {
      if (kind === "abort") {
        await closeWithExit(1);
        return;
      }
      if (kind === "both") {
        if (status.kind !== "ready" || !status.threeWayDiffs || !status.ancestor || !ancestorPath) {
          throw new Error("Take Both is not available — missing ancestor or three-way diff");
        }
        const merged = buildMergedGraphs(
          status.threeWayDiffs,
          status.ancestor.asset.graphs ?? {},
          status.ours.asset.graphs ?? {},
          status.theirs.asset.graphs ?? {},
          selections,
        );
        await applyGraphMerge(ancestorPath, destPath, merged);
        await closeWithExit(0);
        return;
      }
      await applyResolution(kind, oursPath, theirsPath, destPath);
      await closeWithExit(0);
    } catch (e) {
      setStatus({ kind: "error", message: String(e) });
      setResolving(false);
    }
  }

  if (status.kind === "loading") {
    return <div className={styles.loading}>Loading conflict…</div>;
  }
  if (status.kind === "error") {
    return (
      <div className={styles.error}>
        <p>Failed to load:</p>
        <pre>{status.message}</pre>
        <Resolve
          onTakeOurs={() => resolve("ours")}
          onTakeTheirs={() => resolve("theirs")}
          onAbort={() => resolve("abort")}
          disabled={resolving}
        />
      </div>
    );
  }

  const isBlueprint =
    status.ours.asset.class === "Blueprint" ||
    status.theirs.asset.class === "Blueprint";

  const showTakeBoth = isBlueprint && status.threeWayDiffs != null;
  const bothLabel =
    conflictCount > 0
      ? `Take Both (resolve ${conflictCount} conflict${conflictCount === 1 ? "" : "s"})`
      : "Take Both";

  return (
    <div className={styles.container}>
      <header className={styles.header}>
        <span>Conflict: {destPath}</span>
        <span className={styles.summary}>
          {status.changes.length} property change{status.changes.length === 1 ? "" : "s"}
          {" · "}
          ours sha {status.ours.package.savedHash.slice(0, 14)}…
          {" · "}
          theirs sha {status.theirs.package.savedHash.slice(0, 14)}…
        </span>
      </header>

      {isBlueprint && (
        <div className={styles.tabRow}>
          <button
            className={`${styles.tab} ${activeTab === "graph" ? styles.tabActive : ""}`}
            onClick={() => setActiveTab("graph")}
          >
            Graph
          </button>
          <button
            className={`${styles.tab} ${activeTab === "properties" ? styles.tabActive : ""}`}
            onClick={() => setActiveTab("properties")}
          >
            Properties
          </button>
        </div>
      )}

      {(!isBlueprint || activeTab === "properties") && (
        <div className={styles.panes}>
          <PropertiesDiff
            title="Ours"
            properties={status.ours.asset.properties}
            highlight={changedPaths}
          />
          <PropertiesDiff
            title="Theirs"
            properties={status.theirs.asset.properties}
            highlight={changedPaths}
          />
        </div>
      )}

      {isBlueprint && activeTab === "graph" && (
        <GraphView
          ours={status.ours}
          theirs={status.theirs}
          graphDiffs={status.graphDiffs}
          ancestor={status.ancestor}
          threeWayDiffs={status.threeWayDiffs}
          selections={selections}
          onSelectionChange={onSelectionChange}
        />
      )}

      <Resolve
        onTakeOurs={() => resolve("ours")}
        onTakeTheirs={() => resolve("theirs")}
        onTakeBoth={showTakeBoth ? () => resolve("both") : undefined}
        onAbort={() => resolve("abort")}
        disabled={resolving}
        bothLabel={bothLabel}
      />
    </div>
  );
}
