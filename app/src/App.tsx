import { useEffect, useState } from "react";
import { getAppMode } from "./ipc";
import type { AppMode } from "./types";
import ConflictList from "./views/ConflictList";
import Diff from "./views/Diff";
import BlueprintTest from "./views/BlueprintTest";

export default function App() {
  const [mode, setMode] = useState<AppMode | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getAppMode().then(setMode).catch((e) => setError(String(e)));
  }, []);

  // No Tauri context (plain browser / pnpm dev): show blueprint smoke test.
  if (error) {
    return <BlueprintTest />;
  }
  if (!mode) {
    return (
      <main style={{ padding: "1.5rem" }}>
        <h1>Unreal Merge</h1>
        <p>Loading…</p>
      </main>
    );
  }

  if (mode.kind === "gitDriverGui") {
    // In git-driver mode the destination is the working-tree file Git passed
    // as %A — which Tauri receives as the `ours` argv slot. After the driver
    // exits, Git uses this path as the resolved file.
    return <Diff oursPath={mode.ours} theirsPath={mode.theirs} destPath={mode.ours} />;
  }

  if (mode.kind === "standaloneGui") {
    return <ConflictList />;
  }

  // mode.kind === "cli" should never reach the Tauri runtime (main.rs branches
  // off before constructing it), so this is purely defensive.
  return <p>Unexpected CLI mode in GUI runtime.</p>;
}
