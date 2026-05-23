import { invoke } from "@tauri-apps/api/core";
import type { AppMode, AssetSnapshot, PropertyChange, GraphDiff } from "./types";

export async function getAppMode(): Promise<AppMode> {
  return invoke<AppMode>("get_app_mode");
}

export async function exportAsset(
  path: string,
  options?: { sidecarOverride?: string; hostProjectOverride?: string },
): Promise<AssetSnapshot> {
  return invoke<AssetSnapshot>("export_asset", {
    path,
    sidecarOverride: options?.sidecarOverride,
    hostProjectOverride: options?.hostProjectOverride,
  });
}

export async function diffSnapshots(
  ours: AssetSnapshot,
  theirs: AssetSnapshot,
): Promise<PropertyChange[]> {
  return invoke<PropertyChange[]>("diff_snapshots", { ours, theirs });
}

export async function diffGraphs(
  ours: AssetSnapshot,
  theirs: AssetSnapshot,
): Promise<GraphDiff[]> {
  return invoke<GraphDiff[]>("diff_graphs", { ours, theirs });
}

export async function applyResolution(
  resolution: "ours" | "theirs" | "abort",
  oursPath: string,
  theirsPath: string,
  destPath: string,
): Promise<void> {
  await invoke<void>("apply_resolution", {
    resolution,
    ours: oursPath,
    theirs: theirsPath,
    dest: destPath,
  });
}

export async function closeWithExit(code: number): Promise<void> {
  await invoke<void>("close_with_exit", { code });
}
