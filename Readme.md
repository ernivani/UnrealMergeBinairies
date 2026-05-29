# Unreal Merge Binaries

A Git merge driver for Unreal Engine `.uasset` / `.umap` binary files. When `git merge` produces a Blueprint conflict, this tool auto-launches a GUI that diffs both sides (properties + node graphs) and lets you pick a resolution - including **Take Both** (VS Code-style "accept current AND incoming") for non-conflicting graph-node changes.

## Goals

- [x] Resolve `.uasset` Blueprint conflicts visually (Take Ours / Take Theirs)
- [x] **Take Both** - three-way merge of node graphs with per-node conflict picker
- [ ] Same workflow for non-Blueprint `.uasset` types (currently property-diff only, no semantic merge)

## How it works

```
git merge ──► conflict on *.uasset ──► git invokes:
              unreal-merge.exe --git-driver %O %A %B %P
                                         │   │   │   │
                                ancestor─┘   │   │   └─dest path
                                       ours──┘   └─theirs
                                              │
                                              ▼
                                  Tauri GUI opens with side-by-side
                                  diff + Take Ours / Theirs / Both / Abort
```

## Prerequisite: install the commandlet as an engine plugin

To export/rewrite real Blueprints, UE must be able to run the `MergeBinariesExport`
commandlet for **your game project**. Build the plugin once and drop it into the
engine so every project can load it:

```powershell
# Compile the plugin against your installed UE 5.6
& "C:\Program Files\Epic Games\UE_5.6\Engine\Build\BatchFiles\RunUAT.bat" BuildPlugin `
  -Plugin="C:\path\to\UnrealMergeBinairies\ue-host\Plugins\MergeBinariesExport\MergeBinariesExport.uplugin" `
  -Package="$env:TEMP\mbe-build" -TargetPlatforms=Win64 -Rocket

# Copy the result into the engine's plugins (so any .uproject can use it)
$dst = "C:\Program Files\Epic Games\UE_5.6\Engine\Plugins\MergeBinariesExport"
New-Item -ItemType Directory -Force "$dst\Binaries\Win64" | Out-Null
Copy-Item "$env:TEMP\mbe-build\Binaries\Win64\*" "$dst\Binaries\Win64\" -Force
Copy-Item "C:\path\to\UnrealMergeBinairies\ue-host\Plugins\MergeBinariesExport\MergeBinariesExport.uplugin" $dst -Force
```

It is `EnabledByDefault`, so once it's an engine plugin the tool opens your game's
own `.uproject` (auto-detected by walking up from the conflicted asset) and all
parent classes / referenced assets resolve correctly.

> **Path gotcha:** Git invokes the merge driver via `sh -c`, which splits on
> spaces. Put `unreal-merge.exe` somewhere **without spaces** (e.g. `C:\tools\`)
> - a path under `…\Unreal Projects\…` will fail with "No such file or directory".

## First-time setup in a new repo

From the root of the repo where you have `.uasset` conflicts (e.g. `IcanFPS2026`):

```powershell
# 1. Build the release exe (once)
cd path\to\UnrealMergeBinairies\app
pnpm install
pnpm tauri build
# → produces app\src-tauri\target\release\unreal-merge.exe

# 2. Install the driver in your game repo
cd path\to\YourGameRepo
"C:\path\to\UnrealMergeBinairies\app\src-tauri\target\release\unreal-merge.exe" install
```

The `install` step is **idempotent** and modifies two files in your game repo:

- `.gitattributes` - adds `*.uasset merge=unrealbin` and `*.umap merge=unrealbin` between marker lines
- `.git/config` - adds a `[merge "unrealbin"]` section pointing at the absolute path of `unreal-merge.exe`

Commit the `.gitattributes` change so the whole team uses the driver. Each teammate still needs the exe on disk and must run `install` once locally (because `.git/config` is per-clone and stores an absolute path).

## Using it during a merge

```powershell
# In your game repo, with a conflicting .uasset on the index:
git merge feature/new-blueprint
# Auto-launches the GUI for each .uasset / .umap conflict
```

Inside the GUI:

| Button | Effect |
|---|---|
| **Take Ours** | Keep `%A` (current branch). Git marks resolved, exits 0. |
| **Take Theirs** | Keep `%B` (incoming). Git marks resolved, exits 0. |
| **Take Both** | Blueprint-only. Three-way merge of node graphs - non-conflicting changes from both sides auto-accepted; per-node Ours / Theirs / Skip picker for conflicts. Calls back into UE to rewrite the `.uasset`, then git marks resolved. |
| **Abort** | Leave the conflict in place, exit non-zero. Git keeps the working tree at the conflict state. |

The window has two tabs for Blueprints: **Graph** (rendered node graph with diff outlines) and **Properties** (side-by-side property table).

## Picking the UE sidecar

The Rust side spawns a UE 5.6 commandlet (`MergeBinariesExport`) over JSON-RPC to actually export and rewrite `.uasset` bytes. Two ways to point at it:

- **Default (release builds)**: `C:\Program Files\Epic Games\UE_5.6\Engine\Binaries\Win64\UnrealEditor.exe`
- **Override**: set `UNREAL_MERGE_SIDECAR=C:\path\to\UnrealEditor.exe` before launching
- **Debug builds** auto-detect `mock_ue_sidecar.exe` next to `unreal-merge.exe` (canned BP_Base fixtures - UI exercise only, no real `.uasset` export)

The host UE project containing the `MergeBinariesExport` plugin is at `ue-host/HostProject.uproject` in this repo.

## Headless / CI override

For scripted merges where popping a window is unacceptable:

```powershell
$env:UNREAL_MERGE_RESOLUTION = "theirs"   # or "ours" or "abort"
git merge feature/whatever
```

The exe sees the env var and skips the GUI, applying the resolution directly.

## Uninstall

```powershell
cd path\to\YourGameRepo
"C:\path\to\unreal-merge.exe" uninstall
```

Strips the marker block from `.gitattributes` and the `[merge "unrealbin"]` section from `.git/config`.

## Other CLI commands

```powershell
unreal-merge.exe install [--repo PATH]      # install driver
unreal-merge.exe uninstall [--repo PATH]    # remove driver
unreal-merge.exe scan [--repo PATH]         # list unresolved .uasset/.umap conflicts
unreal-merge.exe export PATH                # JSON snapshot of one .uasset (debugging)
unreal-merge.exe diff OURS THEIRS           # property diff between two .uasset files
unreal-merge.exe merge TARGET --graphs G.json --out OUT.uasset   # apply a graph merge headlessly
```

`merge` drives the same writeback as the GUI's **Take Both** button: `TARGET` is
the real asset inside the project, `--graphs` is a JSON map of `{graphName:
nodeText}`, and the merged Blueprint is written to `--out`. Used to test the
writeback without the GUI (round-trip: `export` a Blueprint's graphs, feed them
to `merge`, re-`export` the result - node graphs reproduce identically).

Standalone GUI mode (`unreal-merge.exe` with no args) is a placeholder - the scan-the-repo workflow is deferred. For real conflicts use the git-driver flow above.

## Development

```powershell
# Browser-only dev (no Rust rebuild, mocked data):
cd app && pnpm dev
# → http://127.0.0.1:1420 - renders BP_Base 3-way fixture

# Full Tauri app, mock sidecar:
cd app
pnpm tauri dev -- -- --git-driver `
  "...\Examples\v1\BP_MinimalChar.uasset" `
  "...\Examples\v1\BP_MinimalChar.uasset" `
  "...\Examples\v2\BP_MinimalChar.uasset" `
  "...\Examples\v1\BP_MinimalChar.uasset"

# Full Rust test suite (~60 tests):
cd app && cargo test --manifest-path src-tauri/Cargo.toml
```

See `docs/HANDOFF.md` for architecture details and `docs/superpowers/plans/` for per-plan implementation history.
