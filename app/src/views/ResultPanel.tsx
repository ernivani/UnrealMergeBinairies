import { useMemo } from "react";
import type { MergeSide, ThreeWayGraphDiff, ThreeWayNodeStatus } from "../types";
import { isConflictStatus } from "../types";
import { defaultSide, parseNodeBlobs } from "../mergeGraphs";
import { nodeLabel } from "../nodeLabel";
import styles from "./ResultPanel.module.css";

interface Props {
  diff: ThreeWayGraphDiff;
  oursText?: string;
  theirsText?: string;
  ancestorText?: string;
  selections: Map<string, MergeSide>;
  onSelect: (guid: string, side: MergeSide) => void;
  selectedGuid?: string;
  onRowClick: (guid: string) => void;
}

type Glyph = "added" | "removed" | "modified" | "conflict";
interface SegOption {
  side: MergeSide;
  text: string;
}
interface RowSpec {
  glyph: Glyph;
  options: SegOption[];
}

// Map a three-way status to its glyph + the side-buttons offered for it.
// Side values map directly onto the backend's MergeSide ("ours"|"theirs"|"skip").
function rowSpec(status: ThreeWayNodeStatus): RowSpec | null {
  switch (status) {
    case "unchanged":
    case "removedInBoth":
      return null; // not shown — implicitly kept / dropped
    case "addedInOurs":
    case "addedInBoth":
      return { glyph: "added", options: [{ side: "ours", text: "Keep" }, { side: "skip", text: "Skip" }] };
    case "addedInTheirs":
      return { glyph: "added", options: [{ side: "theirs", text: "Keep" }, { side: "skip", text: "Skip" }] };
    case "removedInTheirs":
      // ours still has the node; Keep = ours, Drop = skip
      return { glyph: "removed", options: [{ side: "ours", text: "Keep" }, { side: "skip", text: "Drop" }] };
    case "removedInOurs":
      // theirs still has the node; Keep = theirs, Drop = skip
      return { glyph: "removed", options: [{ side: "theirs", text: "Keep" }, { side: "skip", text: "Drop" }] };
    case "modifiedInOurs":
    case "modifiedInTheirs":
      return { glyph: "modified", options: [{ side: "ours", text: "Ours" }, { side: "theirs", text: "Theirs" }] };
    case "modifiedInBoth":
    case "addedInBothConflict":
    case "modifyDeleteConflict":
      return {
        glyph: "conflict",
        options: [
          { side: "ours", text: "Ours" },
          { side: "theirs", text: "Theirs" },
          { side: "skip", text: "Skip" },
        ],
      };
  }
}

const glyphChar: Record<Glyph, string> = {
  added: "+",
  removed: "−",
  modified: "~",
  conflict: "!",
};
const glyphClass: Record<Glyph, string> = {
  added: styles.glyphAdded,
  removed: styles.glyphRemoved,
  modified: styles.glyphModified,
  conflict: styles.glyphConflict,
};

export default function ResultPanel({
  diff,
  oursText,
  theirsText,
  ancestorText,
  selections,
  onSelect,
  selectedGuid,
  onRowClick,
}: Props) {
  const rows = useMemo(() => {
    const ours = parseNodeBlobs(oursText ?? "");
    const theirs = parseNodeBlobs(theirsText ?? "");
    const anc = parseNodeBlobs(ancestorText ?? "");
    const out: Array<{ guid: string; status: ThreeWayNodeStatus; spec: RowSpec; label: string; conflict: boolean }> = [];
    for (const [guid, status] of Object.entries(diff.nodeStatuses)) {
      const spec = rowSpec(status as ThreeWayNodeStatus);
      if (!spec) continue;
      const blob = ours.get(guid) ?? theirs.get(guid) ?? anc.get(guid) ?? "";
      out.push({
        guid,
        status: status as ThreeWayNodeStatus,
        spec,
        label: nodeLabel(blob),
        conflict: isConflictStatus(status as ThreeWayNodeStatus),
      });
    }
    // Conflicts first, then by label for stable ordering.
    out.sort((a, b) => Number(b.conflict) - Number(a.conflict) || a.label.localeCompare(b.label));
    return out;
  }, [diff, oursText, theirsText, ancestorText]);

  const conflictCount = rows.filter((r) => r.conflict).length;

  return (
    <div className={styles.panel}>
      <div className={styles.header}>
        <div className={styles.headerTitle}>Result · {diff.name}</div>
        <div className={styles.headerCounts}>
          {rows.length} change{rows.length === 1 ? "" : "s"}
          {" · "}
          {conflictCount === 0 ? (
            <span>no conflicts</span>
          ) : (
            <span className={styles.headerConflicts}>
              {conflictCount} conflict{conflictCount === 1 ? "" : "s"}
            </span>
          )}
        </div>
      </div>
      <div className={styles.list}>
        {rows.length === 0 && <div className={styles.empty}>No changes in this graph.</div>}
        {rows.map((r) => {
          const cur = selections.get(r.guid) ?? defaultSide(r.status);
          const rowCls = [
            styles.row,
            r.conflict ? styles.rowConflict : "",
            selectedGuid === r.guid ? styles.rowSelected : "",
          ]
            .filter(Boolean)
            .join(" ");
          return (
            <div key={r.guid} className={rowCls} onClick={() => onRowClick(r.guid)}>
              <span className={`${styles.glyph} ${glyphClass[r.spec.glyph]}`}>
                {glyphChar[r.spec.glyph]}
              </span>
              <span className={styles.label} title={r.label}>
                {r.label}
              </span>
              <span className={styles.seg}>
                {r.spec.options.map((opt) => {
                  const active = cur === opt.side;
                  const activeCls = active
                    ? opt.side === "ours"
                      ? styles.segBtnActiveOurs
                      : opt.side === "theirs"
                        ? styles.segBtnActiveTheirs
                        : styles.segBtnActiveSkip
                    : "";
                  return (
                    <button
                      key={opt.side}
                      className={`${styles.segBtn} ${activeCls}`}
                      onClick={(e) => {
                        e.stopPropagation();
                        onSelect(r.guid, opt.side);
                      }}
                    >
                      {opt.text}
                    </button>
                  );
                })}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
