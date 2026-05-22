import { useState } from "react";
import styles from "./ConflictList.module.css";
import Diff from "./Diff";

export default function ConflictList() {
  const [oursPath, setOursPath] = useState("");
  const [theirsPath, setTheirsPath] = useState("");
  const [destPath, setDestPath] = useState("");
  const [showing, setShowing] = useState(false);

  if (showing) {
    return <Diff oursPath={oursPath} theirsPath={theirsPath} destPath={destPath} />;
  }

  return (
    <main className={styles.root}>
      <h1>Unreal Merge — Standalone</h1>
      <p className={styles.note}>
        The full scan-the-repo workflow is deferred (Plan 4). For now, paste the
        three paths and click <em>Open</em> to view a diff.
      </p>
      <form
        className={styles.form}
        onSubmit={(e) => {
          e.preventDefault();
          if (oursPath && theirsPath && destPath) setShowing(true);
        }}
      >
        <label>
          Ours (.uasset path)
          <input value={oursPath} onChange={(e) => setOursPath(e.target.value)} />
        </label>
        <label>
          Theirs (.uasset path)
          <input value={theirsPath} onChange={(e) => setTheirsPath(e.target.value)} />
        </label>
        <label>
          Destination (working-tree path)
          <input value={destPath} onChange={(e) => setDestPath(e.target.value)} />
        </label>
        <button type="submit">Open</button>
      </form>
    </main>
  );
}
