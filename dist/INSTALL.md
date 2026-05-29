# Unreal Merge Tool - install (UE 5.6, Windows)

A git merge driver that resolves `.uasset` / `.umap` Blueprint conflicts with a
visual 3-way GUI (per-node Take Both, additive paste).

## Requirements
- Unreal Engine **5.6** (the bundled plugin binary is built for 5.6).
- Windows.

## Install (one time)

1. **Extract this folder to a path with NO spaces** - e.g. `C:\UnrealMergeTool`.
   (Git runs the driver via `sh -c`, which breaks on spaces like `Unreal Projects`.)

2. **Run the installer** (PowerShell) - copies the editor plugin into your engine:
   ```powershell
   cd C:\UnrealMergeTool
   ./install.ps1
   ```
   If your UE 5.6 isn't at the default `C:\Program Files\Epic Games\UE_5.6`, pass it:
   ```powershell
   ./install.ps1 -EnginePath "D:\Epic\UE_5.6"
   ```

3. **Register the driver in your game repo** (each clone, once):
   ```powershell
   cd <your game repo>
   C:\UnrealMergeTool\unreal-merge.exe install
   ```
   This adds a `[merge "unrealbin"]` section to `.git/config`. The matching
   `.gitattributes` (`*.uasset merge=unrealbin`) is already committed in the repo.

## Use

Just merge as normal:
```powershell
git merge dev      # or git pull
```
On a Blueprint conflict the GUI launches automatically:
- **Ours** (left) / **Theirs** (right), common nodes dimmed.
- **✓ Keep / ✕** on each changed node to pick what goes into the result.
- **Take Both** writes the merged asset; **Take Ours/Theirs/Abort** also available.

Then open the asset in UE, **Compile** to sanity-check, and `git commit`.

## Notes
- The exe auto-detects the bundled `ue-host` project (sibling folder) for isolated
  reads, and opens your game's own `.uproject` (found by walking up from the
  conflicted asset) for the merge writeback - so references resolve correctly.
- Headless override (no GUI): `set UNREAL_MERGE_RESOLUTION=ours|theirs|abort` before merging.
- **Different engine version?** Rebuild the plugin from `EnginePlugin/MergeBinariesExport`:
  `& "<UE>\Engine\Build\BatchFiles\RunUAT.bat" BuildPlugin -Plugin="...MergeBinariesExport.uplugin" -Package=out -TargetPlatforms=Win64 -Rocket`,
  then copy the result over `EnginePlugin/MergeBinariesExport` and re-run `install.ps1`.
- Uninstall the driver from a repo: `unreal-merge.exe uninstall`.

## Known limitation
Whole-Blueprint compile is verified after merge. Exotic macro/wildcard nodes are
auto-resolved (For Each Loop etc.), but if a rare node ever flags a type error,
right-click it → **Refresh Node** in the editor, recompile, and commit.
