import { useEffect, useRef, useState } from "react";
import type { GraphDiff, MergeSide, ThreeWayGraphDiff } from "../types";
import { applyDiffOverlay, applyThreeWayOverlay, type PaneSide } from "../graphDiff";
import styles from "./GraphPane.module.css";

interface Props {
  label: string;
  side: PaneSide;
  graphText: string | undefined;
  diff: GraphDiff | undefined;
  threeWayDiff?: ThreeWayGraphDiff;
  selections?: Map<string, MergeSide>;
  /** GUIDs identical on both sides - dimmed as "common/agreed". */
  common?: Set<string>;
}

export default function GraphPane({ label, side, graphText, diff, threeWayDiff, selections, common }: Props) {
  const canvasRef = useRef<HTMLDivElement>(null);
  // True once the ueblueprint web component has rendered its <ueb-node>s.
  const [nodesReady, setNodesReady] = useState(false);

  // Build the canvas ONLY when the graph text changes. Selection/overlay
  // changes must NOT rebuild the (expensive) ueblueprint DOM, or every
  // Keep/Drop click would reload and reset the whole view.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    setNodesReady(false);
    canvas.innerHTML = "";
    if (!graphText) return;

    const escaped = graphText
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
    canvas.innerHTML =
      `<ueb-blueprint style="display:block;width:100%;height:100%;--ueb-height:100%">` +
      `<template>${escaped}</template>` +
      `</ueb-blueprint>`;

    let settled = false;
    const observer = new MutationObserver(() => {
      if (canvas.querySelector("ueb-node") && !settled) {
        settled = true;
        setNodesReady(true);
      }
    });
    observer.observe(canvas, { childList: true, subtree: true });

    return () => {
      observer.disconnect();
      canvas.innerHTML = "";
    };
  }, [graphText]);

  // Apply (or re-apply) the diff overlay + selection dimming. Runs on selection
  // changes too, but only toggles CSS classes on existing nodes - no rebuild.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !nodesReady) return;
    if (threeWayDiff) {
      applyThreeWayOverlay(canvas, threeWayDiff, side, selections ?? new Map(), common ?? new Set());
    } else if (diff && side !== "result") {
      applyDiffOverlay(canvas, diff, side);
    }
  }, [nodesReady, diff, threeWayDiff, selections, common, side]);

  return (
    <div className={styles.pane}>
      <div className={`${styles.label} ${side === "ours" ? styles.ours : ""}`}>{label}</div>
      <div ref={canvasRef} className={styles.canvas} />
      {!graphText && <div className={styles.empty}>No graph data</div>}
    </div>
  );
}
