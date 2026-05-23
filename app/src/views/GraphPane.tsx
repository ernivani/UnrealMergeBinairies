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
    blueprintEl.style.width = "100%";
    blueprintEl.style.height = "100%";
    blueprintEl.style.display = "block";

    const templateEl = document.createElement("template");
    templateEl.innerHTML = graphText;
    blueprintEl.appendChild(templateEl);
    canvas.appendChild(blueprintEl);

    if (diff) {
      requestAnimationFrame(() => {
        applyDiffOverlay(canvas, diff, side);
      });
    }
  }, [graphText, diff, side]);

  return (
    <div className={styles.pane}>
      <div className={`${styles.label} ${side === "ours" ? styles.ours : ""}`}>
        {label}
      </div>
      <div ref={canvasRef} className={styles.canvas}>
        {!graphText && <div className={styles.empty}>No graph data</div>}
      </div>
    </div>
  );
}
