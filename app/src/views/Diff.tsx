import { useEffect, useMemo, useState } from "react";
import { applyResolution, closeWithExit, diffGraphs, diffSnapshots, exportAsset } from "../ipc";
import type { AssetSnapshot, GraphDiff, PropertyChange } from "../types";
import GraphView from "./GraphView";
import PropertiesDiff from "./PropertiesDiff";
import Resolve from "./Resolve";
import styles from "./Diff.module.css";

interface Props {
  oursPath: string;
  theirsPath: string;
  /**
   * Working-tree destination where the resolved file goes. In git-driver
   * mode this is the same as `oursPath` (Git uses %A as both input and
   * destination). In standalone mode the caller passes the real path.
   */
  destPath: string;
}

type Status =
  | { kind: "loading" }
  | { kind: "error"; message: string }
  | {
      kind: "ready";
      ours: AssetSnapshot;
      theirs: AssetSnapshot;
      changes: PropertyChange[];
      graphDiffs: GraphDiff[];
    };

type Tab = "graph" | "properties";

export default function Diff({ oursPath, theirsPath, destPath }: Props) {
  const [status, setStatus] = useState<Status>({ kind: "loading" });
  const [resolving, setResolving] = useState(false);
  const [activeTab, setActiveTab] = useState<Tab>("graph");

  // Reset to Graph tab when a new conflict is opened.
  useEffect(() => {
    setActiveTab("graph");
  }, [oursPath, theirsPath]);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const [ours, theirs] = await Promise.all([
          exportAsset(oursPath),
          exportAsset(theirsPath),
        ]);
        const [changes, graphDiffs] = await Promise.all([
          diffSnapshots(ours, theirs),
          diffGraphs(ours, theirs),
        ]);
        if (!cancelled)
          setStatus({ kind: "ready", ours, theirs, changes, graphDiffs });
      } catch (e) {
        if (!cancelled) setStatus({ kind: "error", message: String(e) });
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, [oursPath, theirsPath]);

  const changedPaths = useMemo(() => {
    if (status.kind !== "ready") return new Set<string>();
    const s = new Set<string>();
    for (const c of status.changes) s.add(c.path);
    return s;
  }, [status]);

  async function resolve(kind: "ours" | "theirs" | "abort") {
    setResolving(true);
    try {
      if (kind === "abort") {
        await closeWithExit(1);
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
        />
      )}

      <Resolve
        onTakeOurs={() => resolve("ours")}
        onTakeTheirs={() => resolve("theirs")}
        onAbort={() => resolve("abort")}
        disabled={resolving}
      />
    </div>
  );
}
