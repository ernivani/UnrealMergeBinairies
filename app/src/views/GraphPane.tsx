import { useEffect, useRef } from "react";
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
  /** GUID of the node to flash/outline (set when a Result row is clicked). */
  selectedGuid?: string;
}

export default function GraphPane({ label, side, graphText, diff, threeWayDiff, selections, common, selectedGuid }: Props) {
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
            applyThreeWayOverlay(canvas, threeWayDiff, side, selections ?? new Map(), common ?? new Set());
          } else if (diff && side !== "result") {
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
  }, [graphText, diff, threeWayDiff, selections, common, side]);

  // Flash/outline the node selected in the Result panel. Separate effect so it
  // toggles without re-rendering the (expensive) blueprint canvas.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    let scrolled = false;
    canvas.querySelectorAll("ueb-node").forEach((el) => {
      const nodeEl = el as HTMLElement & { entity?: { NodeGuid?: { toString(): string } } };
      const guid = nodeEl.entity?.NodeGuid?.toString();
      const match = !!selectedGuid && guid === selectedGuid;
      nodeEl.classList.toggle("uem-selected", match);
      if (match && !scrolled) {
        scrolled = true;
        el.scrollIntoView({ block: "center", inline: "center", behavior: "smooth" });
      }
    });
  }, [selectedGuid, graphText, threeWayDiff]);

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
