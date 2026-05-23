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

    const blueprintEl = document.createElement("ueb-blueprint");

    const templateEl = document.createElement("template");
    templateEl.innerHTML = graphText;
    blueprintEl.appendChild(templateEl);
    canvas.appendChild(blueprintEl);

    if (!diff) return;

    // ueb-blueprint renders ueb-node children asynchronously (Lit). Use a
    // MutationObserver to wait until at least one ueb-node appears before
    // applying the diff overlay — rAF alone fires too early.
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
