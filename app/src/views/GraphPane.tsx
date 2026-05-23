import { useEffect, useRef } from "react";
import type { GraphDiff } from "../types";
import { applyDiffOverlay } from "../graphDiff";
import styles from "./GraphPane.module.css";

interface Props {
  label: string;
  side: "ours" | "theirs";
  graphText: string | undefined;
  diff: GraphDiff | undefined;
}

export default function GraphPane({ label, side, graphText, diff }: Props) {
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

    // ueblueprint's Blueprint constructor sets attributes during construction,
    // which document.createElement() rejects with NotSupportedError. The HTML
    // parser path (innerHTML) tolerates this, so we build via markup.
    // The template text is plain UE serialization, but it contains `<` chars
    // (none — but quotes and = are fine). We escape `<`, `>`, and `&` defensively.
    const escaped = graphText
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
    canvas.innerHTML =
      `<ueb-blueprint style="display:block;width:100%;height:100%;--ueb-height:100%">` +
      `<template>${escaped}</template>` +
      `</ueb-blueprint>`;

    if (!diff) return;

    let rafId: number | undefined;
    const observer = new MutationObserver(() => {
      if (canvas.querySelector("ueb-node")) {
        observer.disconnect();
        rafId = requestAnimationFrame(() => {
          applyDiffOverlay(canvas, diff, side);
        });
      }
    });
    observer.observe(canvas, { childList: true, subtree: true });

    return () => {
      observer.disconnect();
      if (rafId !== undefined) cancelAnimationFrame(rafId);
      canvas.innerHTML = "";
    };
  }, [graphText, diff, side]);

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
