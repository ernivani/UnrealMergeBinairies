import { useEffect, useRef } from "react";
import type { GraphDiff, MergeSide, ThreeWayGraphDiff } from "../types";
import { applyDiffOverlay, applyThreeWayOverlay } from "../graphDiff";
import styles from "./GraphPane.module.css";

interface Props {
  label: string;
  side: "ours" | "theirs";
  graphText: string | undefined;
  diff: GraphDiff | undefined;
  threeWayDiff?: ThreeWayGraphDiff;
  selections?: Map<string, MergeSide>;
}

export default function GraphPane({ label, side, graphText, diff, threeWayDiff, selections }: Props) {
  const canvasRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    canvas.innerHTML = "";
    if (!graphText) return;

    if (!customElements.get("ueb-blueprint")) {
      // eslint-disable-next-line no-console
      console.error("ueb-blueprint custom element not registered");
    }

    const escaped = graphText
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
    canvas.innerHTML =
      `<ueb-blueprint style="display:block;width:100%;height:100%;--ueb-height:100%">` +
      `<template>${escaped}</template>` +
      `</ueb-blueprint>`;

    if (!diff && !threeWayDiff) return;

    let rafId: number | undefined;
    const observer = new MutationObserver(() => {
      if (canvas.querySelector("ueb-node")) {
        observer.disconnect();
        rafId = requestAnimationFrame(() => {
          if (threeWayDiff) {
            applyThreeWayOverlay(canvas, threeWayDiff, side, selections ?? new Map());
          } else if (diff) {
            applyDiffOverlay(canvas, diff, side);
          }
        });
      }
    });
    observer.observe(canvas, { childList: true, subtree: true });

    return () => {
      observer.disconnect();
      if (rafId !== undefined) cancelAnimationFrame(rafId);
      canvas.innerHTML = "";
    };
  }, [graphText, diff, threeWayDiff, selections, side]);

  return (
    <div className={styles.pane}>
      <div className={`${styles.label} ${side === "ours" ? styles.ours : ""}`}>
        {label}
      </div>
      <div ref={canvasRef} className={styles.canvas} />
      {!graphText && <div className={styles.empty}>No graph data</div>}
    </div>
  );
}
