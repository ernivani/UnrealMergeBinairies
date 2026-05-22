import type { Property } from "../types";
import styles from "./PropertiesDiff.module.css";

interface Props {
  title: string;
  properties: Property[];
  /** Property paths that differ from the opposite side. */
  highlight: Set<string>;
}

function formatValue(v: unknown): string {
  if (v === null || v === undefined) return "";
  if (typeof v === "string") return v;
  if (typeof v === "number" || typeof v === "boolean") return String(v);
  // For struct/array/map/set summaries: render compactly.
  return JSON.stringify(v);
}

export default function PropertiesDiff({ title, properties, highlight }: Props) {
  return (
    <section className={styles.pane}>
      <h2 className={styles.title}>{title}</h2>
      <table className={styles.table}>
        <thead>
          <tr>
            <th className={styles.colPath}>Path</th>
            <th className={styles.colType}>Type</th>
            <th className={styles.colValue}>Value</th>
          </tr>
        </thead>
        <tbody>
          {properties.map((p) => (
            <tr
              key={p.path}
              className={highlight.has(p.path) ? styles.changed : undefined}
            >
              <td className={styles.colPath}>{p.path}</td>
              <td className={styles.colType}>{p.type}</td>
              <td className={styles.colValue}>{formatValue(p.value)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
