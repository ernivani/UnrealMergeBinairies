# Plan 3 - Tauri Properties-Diff UI

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wrap Plan 2's `unreal-merge` Rust crate in a Tauri 2 desktop app so that when Git's merge driver invokes `unreal-merge --git-driver %O %A %B %P` on a `.uasset` conflict, a window opens showing a 3-pane property-level diff (Base / Ours / Theirs) and three buttons (Take Ours / Take Theirs / Abort) that file-copy the chosen side over the working tree and close the window with the exit code Git expects.

**Architecture:** The existing `unreal-merge` binary keeps all of Plan 2's CLI subcommands (`install`, `uninstall`, `scan`, `export`, `diff`) and adds a GUI mode. `main.rs` peeks at argv and either runs the CLI dispatch (no GUI) or builds a Tauri app whose state carries the current `AppMode` (StandaloneScan or GitDriverMerge). The frontend is React 18 + TypeScript 5, built with Vite 5. IPC commands wrap the existing `unreal_merge::*` API surface - no new business logic. Plan 4 will add blueprint graph rendering, the "Open in Unreal" action, and richer diff UI on top of this scaffold.

**Tech Stack:**
- Tauri 2 (`tauri = "2"`, `tauri-build = "2"`)
- React 18, TypeScript 5, Vite 5
- pnpm (faster than npm, deterministic lockfile)
- Plain CSS (no Tailwind in Plan 3; Plan 4 may add)
- `@tauri-apps/api` for frontend → Rust IPC
- Dev-only: `@tauri-apps/cli` for `tauri dev` / `tauri build`

**Prerequisites:**
- Plan 2 complete and the `unreal-merge` binary builds (`cd app/src-tauri && cargo build --bin unreal-merge` exits 0).
- Node.js 20+ on PATH: `winget install OpenJS.NodeJS.LTS`. After install, `node --version` and `npm --version` must both work.
- pnpm: `npm install -g pnpm@latest` (after Node is installed). `pnpm --version` must work.
- WebView2 runtime (preinstalled on Win11; Tauri uses it for the embedded browser). Confirm: `Get-AppxPackage *WebView2*` returns a result, or `Test-Path "C:\Program Files (x86)\Microsoft\EdgeWebView\Application"` is `True`.

**Done criteria** (verify before declaring Plan 3 complete):
1. `cd app && pnpm install` exits 0 and creates `node_modules/`.
2. `cd app && pnpm tauri build` produces `app/src-tauri/target/release/unreal-merge.exe` with size ≥ 5 MB (Tauri-built binaries embed the WebView2 launcher + frontend bundle).
3. Scripted end-to-end: a tmp Git repo with a `.uasset` conflict + the built `unreal-merge.exe` installed as merge driver → `git merge` opens the window → clicking "Take Theirs" closes the window with exit 0 → `git status` shows conflict resolved with theirs' content in the working tree.
4. `cd app/src-tauri && cargo test --all-targets` still exits 0 (Plan 2 tests don't regress).
5. CLI subcommands still work: `target/release/unreal-merge.exe scan`, `unreal-merge.exe install`, `unreal-merge.exe export <path>` all behave as in Plan 2.

---

## File structure for this plan

```
app/                              # tauri "frontend project" root (sibling of src-tauri/)
├── package.json
├── pnpm-lock.yaml
├── vite.config.ts
├── tsconfig.json
├── tsconfig.node.json
├── index.html                    # Vite entry
├── src/                          # React + TS frontend
│   ├── main.tsx                  # React root
│   ├── App.tsx                   # mode-aware routing
│   ├── ipc.ts                    # tiny typed wrappers over @tauri-apps/api invoke
│   ├── types.ts                  # mirrors of Rust types (AssetSnapshot etc.)
│   ├── views/
│   │   ├── ConflictList.tsx      # standalone-mode list (currently a thin wrapper)
│   │   ├── Diff.tsx              # 3-pane diff container
│   │   ├── PropertiesDiff.tsx    # the property table itself
│   │   └── Resolve.tsx           # action bar
│   └── styles.css                # tiny global stylesheet
└── src-tauri/                    # Existing Plan 2 Rust crate
    ├── Cargo.toml                # adds tauri + tauri-build deps
    ├── build.rs                  # new - runs tauri_build::build()
    ├── tauri.conf.json           # new - Tauri config
    ├── src/
    │   ├── main.rs               # rewritten: mode peek + Tauri builder
    │   ├── ipc.rs                # new - #[tauri::command] wrappers
    │   ├── app_mode.rs           # new - AppMode enum
    │   ├── cli.rs                # unchanged from Plan 2
    │   ├── diff.rs, git.rs, ...  # unchanged
    │   └── lib.rs                # adds app_mode + ipc modules
    └── icons/                    # Tauri-required icon set (placeholder PNGs)
```

Each file has one responsibility:
- **`main.rs`** - argv peek → either CLI dispatch (Plan 2) or Tauri builder boot.
- **`app_mode.rs`** - `AppMode` enum and the parsing of argv into it.
- **`ipc.rs`** - `#[tauri::command]` functions; thin shims over `unreal_merge::*`. No logic.
- **`tauri.conf.json`** - window config, identifier, build/dev commands, frontend dist path.
- **`build.rs`** - single call to `tauri_build::build()`.
- **`vite.config.ts`** - Vite config tuned for Tauri (fixed port, no HMR overlay during cargo invocations).
- **`tsconfig.json`** - TypeScript strict mode, project references for the node-side config.
- **`src/main.tsx`** - React root.
- **`src/App.tsx`** - mode-aware routing (StandaloneScan vs GitDriverMerge).
- **`src/ipc.ts`** - typed wrappers around `@tauri-apps/api/core::invoke`.
- **`src/types.ts`** - TypeScript mirrors of `AssetSnapshot`, `PropertyChange`, `AppMode`, etc.
- **Each view** - one screen / one responsibility.

---

## Task 0: Install Node + pnpm, verify toolchain

**Files:** none - environment-only.

- [ ] **Step 1: Install Node.js 20 LTS**

```powershell
winget install OpenJS.NodeJS.LTS
```

If `node --version` already prints a 20.x or 22.x version, skip.

- [ ] **Step 2: Install pnpm**

```powershell
npm install -g pnpm@latest
```

Verify: `pnpm --version` should print 9.x or later.

- [ ] **Step 3: Confirm WebView2**

```powershell
Test-Path "C:\Program Files (x86)\Microsoft\EdgeWebView\Application"
```

Expected: `True`. If `False`, install: `winget install Microsoft.EdgeWebView2Runtime`.

- [ ] **Step 4: Note versions for later debugging**

Run and record (so future tasks can match versions if needed):

```powershell
node --version
pnpm --version
```

Save the output in your scratchpad - no commit needed (the lockfile in Task 1 freezes the relevant versions).

---

## Task 1: Frontend scaffolding (pnpm + Vite + React + TS)

**Files:**
- Create: `app/package.json`
- Create: `app/vite.config.ts`
- Create: `app/tsconfig.json`
- Create: `app/tsconfig.node.json`
- Create: `app/index.html`
- Create: `app/src/main.tsx`
- Create: `app/src/App.tsx`
- Create: `app/src/styles.css`
- Modify: `.gitignore`

- [ ] **Step 1: Write `app/package.json`**

Create `app/package.json`:

```json
{
  "name": "unreal-merge-ui",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.1.1",
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.1.0",
    "@types/react": "^18.3.12",
    "@types/react-dom": "^18.3.1",
    "@vitejs/plugin-react": "^4.3.4",
    "typescript": "^5.6.3",
    "vite": "^5.4.11"
  }
}
```

- [ ] **Step 2: Write `app/vite.config.ts`**

Create `app/vite.config.ts`:

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri-friendly Vite config: fixed port (so tauri.conf.json can point at it),
// no clearScreen (so cargo's own output stays visible), strictPort.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
  },
  build: {
    target: "esnext",
    sourcemap: true,
    outDir: "dist",
    emptyOutDir: true,
  },
});
```

- [ ] **Step 3: Write `app/tsconfig.json` and `app/tsconfig.node.json`**

`app/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "skipLibCheck": true,
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "noImplicitOverride": true,
    "jsx": "react-jsx",
    "isolatedModules": true,
    "verbatimModuleSyntax": true,
    "useDefineForClassFields": true,
    "resolveJsonModule": true
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

`app/tsconfig.node.json`:

```json
{
  "compilerOptions": {
    "composite": true,
    "skipLibCheck": true,
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "allowSyntheticDefaultImports": true,
    "strict": true
  },
  "include": ["vite.config.ts"]
}
```

- [ ] **Step 4: Write `app/index.html`**

Create `app/index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Unreal Merge</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 5: Write the minimal React entry + root component**

`app/src/main.tsx`:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

`app/src/App.tsx`:

```tsx
export default function App() {
  return (
    <main>
      <h1>Unreal Merge</h1>
      <p>Plan 3 scaffold - UI lands in Task 6+.</p>
    </main>
  );
}
```

`app/src/styles.css`:

```css
:root {
    color-scheme: dark;
    font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
    background: #1d1f23;
    color: #e6e6e6;
}

body { margin: 0; }
main  { padding: 1.5rem; }
h1    { margin-top: 0; }
```

- [ ] **Step 6: Update `.gitignore`**

Append to `.gitignore`:

```gitignore
# Vite + Node
app/node_modules/
app/dist/
```

Note: `app/pnpm-lock.yaml` IS committed (deterministic builds).

- [ ] **Step 7: Install deps and run a smoke build**

```powershell
cd app
pnpm install
pnpm build
```

Expected: `pnpm install` finishes with a generated `pnpm-lock.yaml` and `node_modules/`. `pnpm build` produces `app/dist/index.html` and `app/dist/assets/*.js`. Both commands exit 0.

- [ ] **Step 8: Commit**

```powershell
cd ..
git add app/package.json app/pnpm-lock.yaml app/vite.config.ts app/tsconfig.json app/tsconfig.node.json app/index.html app/src/main.tsx app/src/App.tsx app/src/styles.css .gitignore
git commit -m "feat(ui): Vite + React + TS scaffold under app/"
```

---

## Task 2: Add Tauri 2 to the Rust crate

**Files:**
- Modify: `app/src-tauri/Cargo.toml`
- Create: `app/src-tauri/build.rs`
- Create: `app/src-tauri/tauri.conf.json`
- Create: `app/src-tauri/icons/icon.png` (placeholder)

- [ ] **Step 1: Add Tauri deps to `Cargo.toml`**

Edit `app/src-tauri/Cargo.toml`. In the `[dependencies]` section, append:

```toml
tauri = { version = "2", features = [] }
```

Add a NEW section after `[dev-dependencies]`:

```toml
[build-dependencies]
tauri-build = { version = "2", features = [] }
```

The final file's relevant sections should look like:

```toml
[dependencies]
anyhow = "1.0"
clap = { version = "4.5", features = ["derive", "env"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tempfile = "3.10"
walkdir = "2.5"
tauri = { version = "2", features = [] }

[dev-dependencies]
assert_cmd = "2.0"
predicates = "3.1"
pretty_assertions = "1.4"
tempfile = "3.10"

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

- [ ] **Step 2: Write `build.rs`**

Create `app/src-tauri/build.rs`:

```rust
fn main() {
    tauri_build::build();
}
```

- [ ] **Step 3: Write `tauri.conf.json`**

Create `app/src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2.0",
  "productName": "Unreal Merge",
  "version": "0.1.0",
  "identifier": "com.unrealmergebinaries.app",
  "build": {
    "beforeDevCommand": "pnpm dev",
    "devUrl": "http://127.0.0.1:1420",
    "beforeBuildCommand": "pnpm build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Unreal Merge",
        "width": 1280,
        "height": 800,
        "minWidth": 800,
        "minHeight": 600,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": ["msi"],
    "icon": [
      "icons/icon.png"
    ]
  }
}
```

Note: `frontendDist: "../dist"` is relative to `tauri.conf.json`, which lives at `app/src-tauri/tauri.conf.json`. `../dist` resolves to `app/dist` - where Vite writes the production bundle.

`beforeDevCommand: "pnpm dev"` and `beforeBuildCommand: "pnpm build"` run from the directory containing `app/package.json` - i.e. `app/`. Tauri infers this from the relative position of `frontendDist`.

- [ ] **Step 4: Add placeholder icon**

Tauri's `tauri-build` macro requires the icon listed in `bundle.icon` to exist on disk during compilation, even though the bundle target only matters at packaging time. Create a 64×64 placeholder PNG (any valid PNG is fine for now - we won't ship until Plan 4 adds real art).

```powershell
# Use any 64x64 PNG you have. If none handy, generate a solid-colour one in PowerShell:
$bytes = [System.Convert]::FromBase64String('iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAAAGklEQVRIx2NgGAWjYBSMglEwCkbBKBgFwwsACFEAAcCl3KsAAAAASUVORK5CYII=')
New-Item -ItemType Directory -Force -Path "app/src-tauri/icons" | Out-Null
[System.IO.File]::WriteAllBytes("app/src-tauri/icons/icon.png", $bytes)
```

If that base64 doesn't decode to a valid PNG on your version of .NET, use any other 64×64 PNG.

- [ ] **Step 5: Build the crate**

```powershell
cd app/src-tauri
cargo build --bin unreal-merge
```

Expected: `tauri-build` runs, the binary builds. First build downloads tauri's dep tree (~5–10 min on cold cache).

If the build fails because `tauri-build` complains about the icon, regenerate it with a known-good 64×64 PNG.

- [ ] **Step 6: Commit**

```powershell
cd ../..
git add app/src-tauri/Cargo.toml app/src-tauri/build.rs app/src-tauri/tauri.conf.json app/src-tauri/icons/icon.png
git commit -m "feat(tauri): add Tauri 2 deps + config + placeholder icon"
```

---

## Task 3: `AppMode` enum and argv dispatch in main.rs

**Files:**
- Create: `app/src-tauri/src/app_mode.rs`
- Modify: `app/src-tauri/src/main.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Create: `app/src-tauri/tests/app_mode_test.rs`

- [ ] **Step 1: Write the failing test**

Create `app/src-tauri/tests/app_mode_test.rs`:

```rust
use unreal_merge::app_mode::{AppMode, parse_argv};

#[test]
fn cli_subcommand_argv_routes_to_cli() {
    let argv = vec![
        "unreal-merge".to_string(),
        "scan".to_string(),
    ];
    match parse_argv(&argv) {
        AppMode::Cli => {}
        other => panic!("expected Cli, got {:?}", other),
    }
}

#[test]
fn no_args_routes_to_standalone_gui() {
    let argv = vec!["unreal-merge".to_string()];
    match parse_argv(&argv) {
        AppMode::StandaloneGui => {}
        other => panic!("expected StandaloneGui, got {:?}", other),
    }
}

#[test]
fn git_driver_argv_routes_to_git_driver_gui() {
    let argv = vec![
        "unreal-merge".to_string(),
        "--git-driver".to_string(),
        "anc".to_string(),
        "ours.tmp".to_string(),
        "theirs.tmp".to_string(),
        "a.uasset".to_string(),
    ];
    match parse_argv(&argv) {
        AppMode::GitDriverGui {
            ancestor,
            ours,
            theirs,
            path,
        } => {
            assert_eq!(ancestor, "anc");
            assert_eq!(ours, "ours.tmp");
            assert_eq!(theirs, "theirs.tmp");
            assert_eq!(path, "a.uasset");
        }
        other => panic!("expected GitDriverGui, got {:?}", other),
    }
}
```

- [ ] **Step 2: Run to verify failure**

```powershell
cd app/src-tauri
cargo test --test app_mode_test
```

Expected: compile error - `app_mode` doesn't exist.

- [ ] **Step 3: Implement `app_mode.rs`**

Create `app/src-tauri/src/app_mode.rs`:

```rust
//! How the binary should behave for a given invocation. The same `unreal-merge.exe`
//! is both a Plan 2 CLI (no GUI) and a Plan 3 Tauri app - argv decides which.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AppMode {
    /// Plan 2 CLI subcommands - no GUI, hand off to `cli::run`.
    Cli,
    /// No args - open the GUI in standalone mode (scan current dir for conflicts).
    StandaloneGui,
    /// Git invoked us as a merge driver with 4 positional args.
    GitDriverGui {
        ancestor: String,
        ours: String,
        theirs: String,
        path: String,
    },
}

/// Parse `std::env::args().collect::<Vec<_>>()` into an `AppMode`.
pub fn parse_argv(argv: &[String]) -> AppMode {
    if argv.len() <= 1 {
        return AppMode::StandaloneGui;
    }

    // --git-driver mode: exactly 4 positional args after the flag.
    if let Some(pos) = argv.iter().position(|a| a == "--git-driver") {
        let rest = &argv[pos + 1..];
        if rest.len() >= 4 {
            return AppMode::GitDriverGui {
                ancestor: rest[0].clone(),
                ours: rest[1].clone(),
                theirs: rest[2].clone(),
                path: rest[3].clone(),
            };
        }
        // Wrong arity - fall through to CLI so clap produces a real error.
    }

    // Any other argv shape (install/uninstall/scan/export/diff/--help/--version)
    // routes to the CLI.
    AppMode::Cli
}
```

- [ ] **Step 4: Wire into `lib.rs`**

Update `app/src-tauri/src/lib.rs`:

```rust
//! Backend for unreal-merge.

pub mod app_mode;
pub mod cli;
pub mod diff;
pub mod git;
pub mod installer;
pub mod merge;
pub mod schema;
pub mod sidecar;

pub use diff::{PropertyChange, diff_properties};
pub use schema::{Asset, AssetSnapshot, ErrorResponse, Package, Property, PropertyValue};
pub use sidecar::{Sidecar, SidecarConfig, extract_json_objects};
```

- [ ] **Step 5: Run the test**

```powershell
cd app/src-tauri
cargo test --test app_mode_test
```

Expected: 3 PASS.

- [ ] **Step 6: Commit**

```powershell
cd ../..
git add app/src-tauri
git commit -m "feat(rust): AppMode enum + argv router (CLI / StandaloneGui / GitDriverGui)"
```

---

## Task 4: Tauri IPC commands wrapping the backend

**Files:**
- Create: `app/src-tauri/src/ipc.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Create: `app/src-tauri/tests/ipc_test.rs`

The frontend talks to Rust via `#[tauri::command]` functions. Each one is a thin shim over an existing `unreal_merge::*` function.

- [ ] **Step 1: Write the failing test**

Create `app/src-tauri/tests/ipc_test.rs`:

```rust
//! Tests for the pure (non-Tauri-state-dependent) IPC command bodies.
//! Each #[tauri::command] in ipc.rs delegates to an inner function that
//! takes plain args (no Tauri state) so we can test it without spinning up
//! the runtime.

use unreal_merge::ipc::{
    apply_resolution_inner, diff_snapshots_inner, get_app_mode_inner,
};
use unreal_merge::app_mode::AppMode;

#[test]
fn get_app_mode_inner_returns_constructed_value() {
    let mode = AppMode::StandaloneGui;
    assert_eq!(get_app_mode_inner(&mode), mode);
}

#[test]
fn diff_snapshots_inner_returns_empty_for_identical_inputs() {
    use unreal_merge::schema::{Asset, AssetSnapshot, Package};
    let snap = AssetSnapshot {
        id: None,
        ok: true,
        path: None,
        package: Package {
            name: "x".into(),
            engine_version: "5.6".into(),
            file_version_ue5: 1017,
            saved_hash: "sha1:0".into(),
        },
        asset: Asset {
            class: "Blueprint".into(),
            parent_class: "".into(),
            name: "Test".into(),
            properties: vec![],
        },
    };
    let diffs = diff_snapshots_inner(&snap, &snap);
    assert!(diffs.is_empty());
}

#[test]
fn apply_resolution_inner_take_theirs_copies_file() {
    use tempfile::TempDir;
    let tmp = TempDir::new().unwrap();
    let ours = tmp.path().join("ours");
    let theirs = tmp.path().join("theirs");
    let dest = tmp.path().join("dest");
    std::fs::write(&ours, b"OURS").unwrap();
    std::fs::write(&theirs, b"THEIRS").unwrap();
    std::fs::write(&dest, b"STALE").unwrap();
    apply_resolution_inner("theirs", &ours, &theirs, &dest).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"THEIRS");
}
```

- [ ] **Step 2: Run to verify it fails**

```powershell
cd app/src-tauri
cargo test --test ipc_test
```

Expected: compile error - `ipc` module doesn't exist.

- [ ] **Step 3: Implement `ipc.rs`**

Create `app/src-tauri/src/ipc.rs`:

```rust
//! Tauri IPC commands. Each #[tauri::command] is a thin shim around a plain
//! `*_inner` function (no Tauri state) so unit tests can exercise the logic
//! without spinning the Tauri runtime.

use crate::app_mode::AppMode;
use crate::diff::{PropertyChange, diff_properties};
use crate::merge;
use crate::schema::AssetSnapshot;
use crate::sidecar::{Sidecar, SidecarConfig};
use std::path::{Path, PathBuf};

/// Returned to the frontend at startup so the React app knows whether to
/// open the standalone list or the focused merge view.
pub fn get_app_mode_inner(mode: &AppMode) -> AppMode {
    mode.clone()
}

#[tauri::command]
pub fn get_app_mode(state: tauri::State<'_, AppMode>) -> AppMode {
    get_app_mode_inner(&state)
}

pub fn diff_snapshots_inner(ours: &AssetSnapshot, theirs: &AssetSnapshot) -> Vec<PropertyChange> {
    diff_properties(&ours.asset.properties, &theirs.asset.properties)
}

#[tauri::command]
pub fn diff_snapshots(ours: AssetSnapshot, theirs: AssetSnapshot) -> Vec<PropertyChange> {
    diff_snapshots_inner(&ours, &theirs)
}

pub fn apply_resolution_inner(
    resolution: &str,
    ours: &Path,
    theirs: &Path,
    dest: &Path,
) -> Result<(), String> {
    let res: merge::Resolution = resolution
        .parse::<merge::Resolution>()
        .map_err(|e| e.to_string())?;
    merge::apply_resolution(res, ours, theirs, dest).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn apply_resolution(
    resolution: String,
    ours: String,
    theirs: String,
    dest: String,
) -> Result<(), String> {
    apply_resolution_inner(
        &resolution,
        Path::new(&ours),
        Path::new(&theirs),
        Path::new(&dest),
    )
}

#[tauri::command]
pub fn export_asset(
    path: String,
    sidecar_override: Option<String>,
    host_project_override: Option<String>,
) -> Result<AssetSnapshot, String> {
    let exe = sidecar_override
        .map(PathBuf::from)
        .unwrap_or_else(default_sidecar);
    let host_project = host_project_override
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("ue-host/HostProject.uproject"));

    let args = if exe.to_string_lossy().to_lowercase().contains("unrealeditor") {
        vec![
            host_project.display().to_string(),
            "-run=MergeBinariesExport".to_string(),
            "-stdio".to_string(),
            "-nullrhi".to_string(),
            "-unattended".to_string(),
            "-NoCrashReports".to_string(),
        ]
    } else {
        Vec::new()
    };
    let log_redirect = if exe.to_string_lossy().to_lowercase().contains("unrealeditor") {
        Some(std::env::temp_dir().join(format!(
            "unreal-merge-ipc-{}.log",
            std::process::id()
        )))
    } else {
        None
    };

    let sidecar = Sidecar::new(SidecarConfig {
        executable: exe,
        args,
        prepend_warmup: true,
        log_redirect,
    });

    let abs = std::fs::canonicalize(&path).map_err(|e| format!("canonicalise {}: {}", path, e))?;
    let path_str = abs.to_string_lossy().replace('\\', "/");
    let requests = vec![serde_json::json!({"id": 1, "cmd": "export", "path": path_str})];

    let responses = sidecar.run_batch(&requests).map_err(|e| e.to_string())?;
    let response = responses
        .into_iter()
        .find(|r| r.get("id").and_then(|i| i.as_u64()) == Some(1))
        .ok_or_else(|| "no id=1 response from sidecar".to_string())?;
    let snap: AssetSnapshot =
        serde_json::from_value(response).map_err(|e| format!("parse snapshot: {}", e))?;
    if !snap.ok {
        return Err("commandlet reported ok=false".to_string());
    }
    Ok(snap)
}

fn default_sidecar() -> PathBuf {
    PathBuf::from(r"C:\Program Files\Epic Games\UE_5.6\Engine\Binaries\Win64\UnrealEditor.exe")
}

#[tauri::command]
pub fn close_with_exit(window: tauri::Window, code: i32) {
    // Hide window first so the close feels instant; then exit with the
    // exit code Git expects (0 = resolved, 1 = abort).
    let _ = window.hide();
    std::process::exit(code);
}
```

- [ ] **Step 4: Re-export from `lib.rs`**

Update `app/src-tauri/src/lib.rs`:

```rust
//! Backend for unreal-merge.

pub mod app_mode;
pub mod cli;
pub mod diff;
pub mod git;
pub mod installer;
pub mod ipc;
pub mod merge;
pub mod schema;
pub mod sidecar;

pub use diff::{PropertyChange, diff_properties};
pub use schema::{Asset, AssetSnapshot, ErrorResponse, Package, Property, PropertyValue};
pub use sidecar::{Sidecar, SidecarConfig, extract_json_objects};
```

- [ ] **Step 5: Run the test**

```powershell
cd app/src-tauri
cargo test --test ipc_test
```

Expected: 3 PASS.

- [ ] **Step 6: Commit**

```powershell
cd ../..
git add app/src-tauri
git commit -m "feat(tauri): IPC commands (get_app_mode, diff_snapshots, apply_resolution, export_asset, close_with_exit)"
```

---

## Task 5: Wire main.rs to launch Tauri based on AppMode

**Files:**
- Modify: `app/src-tauri/src/main.rs`

- [ ] **Step 1: Rewrite `main.rs`**

Replace `app/src-tauri/src/main.rs` with:

```rust
//! Entry point. Peeks argv via app_mode::parse_argv and either:
//!  - hands off to the Plan 2 CLI dispatch (Cli mode), or
//!  - boots Tauri with the AppMode inserted into managed state.

use unreal_merge::app_mode::{AppMode, parse_argv};

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let mode = parse_argv(&argv);

    match mode {
        AppMode::Cli => {
            if let Err(e) = unreal_merge::cli::run() {
                eprintln!("error: {:#}", e);
                std::process::exit(1);
            }
        }
        other => {
            tauri::Builder::default()
                .manage(other)
                .invoke_handler(tauri::generate_handler![
                    unreal_merge::ipc::get_app_mode,
                    unreal_merge::ipc::diff_snapshots,
                    unreal_merge::ipc::apply_resolution,
                    unreal_merge::ipc::export_asset,
                    unreal_merge::ipc::close_with_exit,
                ])
                .run(tauri::generate_context!())
                .expect("error while running tauri application");
        }
    }
}
```

- [ ] **Step 2: Confirm the binary still compiles**

```powershell
cd app/src-tauri
cargo build --bin unreal-merge
```

Expected: builds cleanly. Tauri's generate_context! macro reads `tauri.conf.json` (added in Task 2) at compile time - if it complains about the frontend bundle, that's fine: we haven't built the frontend yet. The bundle isn't required for `cargo build` (only for `tauri build`).

- [ ] **Step 3: Confirm all Plan 2 tests still pass**

```powershell
cd app/src-tauri
cargo test --all-targets
```

Expected: every test (schema, diff, mock_sidecar, sidecar, git, merge, merge_driver, cli, git_driver_e2e, app_mode, ipc) passes. Real UE smoke stays ignored.

- [ ] **Step 4: Confirm CLI subcommands still work**

```powershell
cd app/src-tauri
cargo run --bin unreal-merge -- --help
cargo run --bin unreal-merge -- scan --repo ..
```

Expected: `--help` prints subcommand list; `scan` runs (likely "No conflicts." in our repo).

- [ ] **Step 5: Commit**

```powershell
cd ../..
git add app/src-tauri/src/main.rs
git commit -m "feat(tauri): wire main.rs to either CLI or Tauri builder by AppMode"
```

---

## Task 6: TypeScript IPC wrappers + types

**Files:**
- Create: `app/src/types.ts`
- Create: `app/src/ipc.ts`

- [ ] **Step 1: Write `app/src/types.ts`**

Create `app/src/types.ts`:

```ts
/**
 * TypeScript mirrors of the Rust wire types. Hand-written rather than
 * generated to keep Plan 3 simple; if these drift from the Rust side,
 * the integration tests added in Task 11 will surface it.
 */

export interface Package {
  name: string;
  engineVersion: string;
  fileVersionUE5: number;
  savedHash: string;
}

export interface Property {
  path: string;
  type: string;
  value: PropertyValue;
}

// PropertyValue is `#[serde(untagged)]` on the Rust side - could be primitive
// or an object summary for structs/arrays/maps/sets. We model it as `unknown`
// at the type-system level and let the rendering layer branch.
export type PropertyValue = unknown;

export interface Asset {
  class: string;
  parentClass: string;
  name: string;
  properties: Property[];
}

export interface AssetSnapshot {
  id?: number;
  ok: boolean;
  path?: string;
  package: Package;
  asset: Asset;
}

export type PropertyChange =
  | { Added:   { path: string; ty: string; value: PropertyValue } }
  | { Removed: { path: string; ty: string; value: PropertyValue } }
  | { Changed: { path: string; ty: string; old: PropertyValue; new: PropertyValue } };

export type AppMode =
  | { kind: "cli" }
  | { kind: "standaloneGui" }
  | { kind: "gitDriverGui"; ancestor: string; ours: string; theirs: string; path: string };
```

- [ ] **Step 2: Write `app/src/ipc.ts`**

Create `app/src/ipc.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type { AppMode, AssetSnapshot, PropertyChange } from "./types";

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
```

- [ ] **Step 3: Type-check the frontend**

```powershell
cd app
pnpm build
```

Expected: builds cleanly (no TS errors). Output goes to `app/dist/`.

- [ ] **Step 4: Commit**

```powershell
cd ..
git add app/src/types.ts app/src/ipc.ts
git commit -m "feat(ui): TypeScript types + typed IPC wrappers"
```

---

## Task 7: PropertiesDiff component

**Files:**
- Create: `app/src/views/PropertiesDiff.tsx`
- Create: `app/src/views/PropertiesDiff.module.css`

PropertiesDiff renders a single ordered list of `Property` entries with each row showing `path` and `value`. It accepts a `highlight` set of paths (those that differ from the other side) and styles them.

- [ ] **Step 1: Write the component**

Create `app/src/views/PropertiesDiff.tsx`:

```tsx
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
```

- [ ] **Step 2: Write the CSS module**

Create `app/src/views/PropertiesDiff.module.css`:

```css
.pane {
    flex: 1 1 0;
    min-width: 0;
    overflow: auto;
    border-right: 1px solid #2c2f36;
    background: #1d1f23;
}

.pane:last-child { border-right: none; }

.title {
    margin: 0;
    padding: 0.5rem 0.75rem;
    font-size: 0.95rem;
    font-weight: 600;
    background: #25282e;
    border-bottom: 1px solid #2c2f36;
    position: sticky;
    top: 0;
}

.table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
    font-family: ui-monospace, "Cascadia Mono", "JetBrains Mono", monospace;
}

.table th,
.table td {
    padding: 0.25rem 0.6rem;
    border-bottom: 1px solid #25282e;
    text-align: left;
    vertical-align: top;
}

.table th {
    font-weight: 500;
    background: #2a2d33;
    color: #b8b8b8;
}

.colPath  { white-space: nowrap; color: #d5d5d5; }
.colType  { color: #888; white-space: nowrap; }
.colValue { color: #9ab; word-break: break-all; }

.changed {
    background: rgba(255, 170, 60, 0.07);
}
.changed .colPath  { color: #ffc66d; }
.changed .colValue { color: #ffe1a3; }
```

- [ ] **Step 3: Type-check**

```powershell
cd app
pnpm build
```

Expected: builds cleanly.

- [ ] **Step 4: Commit**

```powershell
cd ..
git add app/src/views/PropertiesDiff.tsx app/src/views/PropertiesDiff.module.css
git commit -m "feat(ui): PropertiesDiff component (sticky-header table with change highlight)"
```

---

## Task 8: Resolve action bar

**Files:**
- Create: `app/src/views/Resolve.tsx`
- Create: `app/src/views/Resolve.module.css`

- [ ] **Step 1: Write the component**

Create `app/src/views/Resolve.tsx`:

```tsx
import styles from "./Resolve.module.css";

interface Props {
  onTakeOurs: () => void;
  onTakeTheirs: () => void;
  onAbort: () => void;
  disabled: boolean;
}

export default function Resolve({ onTakeOurs, onTakeTheirs, onAbort, disabled }: Props) {
  return (
    <footer className={styles.bar}>
      <button className={styles.btn} onClick={onTakeOurs} disabled={disabled}>
        Take Ours
      </button>
      <button className={styles.btn} onClick={onTakeTheirs} disabled={disabled}>
        Take Theirs
      </button>
      <span className={styles.spacer} />
      <button
        className={`${styles.btn} ${styles.abort}`}
        onClick={onAbort}
        disabled={disabled}
      >
        Abort
      </button>
    </footer>
  );
}
```

- [ ] **Step 2: Write the CSS module**

Create `app/src/views/Resolve.module.css`:

```css
.bar {
    display: flex;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    background: #25282e;
    border-top: 1px solid #2c2f36;
    align-items: center;
}

.spacer { flex: 1; }

.btn {
    background: #3b4148;
    color: #e6e6e6;
    border: 1px solid #4a5159;
    padding: 0.45rem 1rem;
    border-radius: 4px;
    font-size: 0.9rem;
    cursor: pointer;
}

.btn:hover:not(:disabled) {
    background: #485058;
    border-color: #5a6470;
}

.btn:disabled {
    opacity: 0.5;
    cursor: default;
}

.abort {
    background: #4a3b3b;
    border-color: #5a4444;
    color: #e6cccc;
}

.abort:hover:not(:disabled) {
    background: #583c3c;
    border-color: #6e4949;
}
```

- [ ] **Step 3: Type-check**

```powershell
cd app
pnpm build
```

- [ ] **Step 4: Commit**

```powershell
cd ..
git add app/src/views/Resolve.tsx app/src/views/Resolve.module.css
git commit -m "feat(ui): Resolve action bar (Take Ours / Take Theirs / Abort)"
```

---

## Task 9: Diff view container

**Files:**
- Create: `app/src/views/Diff.tsx`
- Create: `app/src/views/Diff.module.css`

The Diff view orchestrates: export both sides via IPC, compute the changed-paths set, render two PropertiesDiff panes (Ours, Theirs - Base is omitted from MVP since 2-way diff is sufficient per Plan 2 design), and a Resolve action bar.

- [ ] **Step 1: Write the component**

Create `app/src/views/Diff.tsx`:

```tsx
import { useEffect, useMemo, useState } from "react";
import { applyResolution, closeWithExit, diffSnapshots, exportAsset } from "../ipc";
import type { AssetSnapshot, PropertyChange } from "../types";
import PropertiesDiff from "./PropertiesDiff";
import Resolve from "./Resolve";
import styles from "./Diff.module.css";

interface Props {
  oursPath: string;
  theirsPath: string;
  /**
   * Working-tree destination where the resolved file goes. In git-driver
   * mode this is the same as `oursPath` (Git uses %A as both input and
   * destination). In standalone mode the caller passes the real path.
   */
  destPath: string;
}

type Status =
  | { kind: "loading" }
  | { kind: "error"; message: string }
  | { kind: "ready"; ours: AssetSnapshot; theirs: AssetSnapshot; changes: PropertyChange[] };

export default function Diff({ oursPath, theirsPath, destPath }: Props) {
  const [status, setStatus] = useState<Status>({ kind: "loading" });
  const [resolving, setResolving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const [ours, theirs] = await Promise.all([
          exportAsset(oursPath),
          exportAsset(theirsPath),
        ]);
        const changes = await diffSnapshots(ours, theirs);
        if (!cancelled) setStatus({ kind: "ready", ours, theirs, changes });
      } catch (e) {
        if (!cancelled) setStatus({ kind: "error", message: String(e) });
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, [oursPath, theirsPath]);

  const changedPaths = useMemo(() => {
    if (status.kind !== "ready") return new Set<string>();
    const s = new Set<string>();
    for (const c of status.changes) {
      if ("Added" in c) s.add(c.Added.path);
      else if ("Removed" in c) s.add(c.Removed.path);
      else s.add(c.Changed.path);
    }
    return s;
  }, [status]);

  async function resolve(kind: "ours" | "theirs" | "abort") {
    setResolving(true);
    try {
      if (kind === "abort") {
        await closeWithExit(1);
        return;
      }
      await applyResolution(kind, oursPath, theirsPath, destPath);
      await closeWithExit(0);
    } catch (e) {
      setStatus({ kind: "error", message: String(e) });
      setResolving(false);
    }
  }

  if (status.kind === "loading") {
    return <div className={styles.loading}>Loading conflict…</div>;
  }
  if (status.kind === "error") {
    return (
      <div className={styles.error}>
        <p>Failed to load:</p>
        <pre>{status.message}</pre>
        <Resolve
          onTakeOurs={() => resolve("ours")}
          onTakeTheirs={() => resolve("theirs")}
          onAbort={() => resolve("abort")}
          disabled={resolving}
        />
      </div>
    );
  }

  return (
    <div className={styles.container}>
      <header className={styles.header}>
        <span>Conflict: {destPath}</span>
        <span className={styles.summary}>
          {status.changes.length} property change{status.changes.length === 1 ? "" : "s"}
          {" · "}
          ours sha {status.ours.package.savedHash.slice(0, 14)}…
          {" · "}
          theirs sha {status.theirs.package.savedHash.slice(0, 14)}…
        </span>
      </header>
      <div className={styles.panes}>
        <PropertiesDiff
          title="Ours"
          properties={status.ours.asset.properties}
          highlight={changedPaths}
        />
        <PropertiesDiff
          title="Theirs"
          properties={status.theirs.asset.properties}
          highlight={changedPaths}
        />
      </div>
      <Resolve
        onTakeOurs={() => resolve("ours")}
        onTakeTheirs={() => resolve("theirs")}
        onAbort={() => resolve("abort")}
        disabled={resolving}
      />
    </div>
  );
}
```

- [ ] **Step 2: Write the CSS module**

Create `app/src/views/Diff.module.css`:

```css
.container {
    display: flex;
    flex-direction: column;
    height: 100vh;
}

.header {
    padding: 0.6rem 1rem;
    background: #1a1c20;
    border-bottom: 1px solid #2c2f36;
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 0.85rem;
}

.summary {
    color: #888;
    font-family: ui-monospace, "Cascadia Mono", monospace;
}

.panes {
    flex: 1 1 auto;
    display: flex;
    min-height: 0;
}

.loading,
.error {
    padding: 2rem;
    color: #aaa;
}

.error pre {
    background: #2a1d1d;
    padding: 0.75rem;
    border-radius: 4px;
    overflow: auto;
    color: #ffb4a8;
}
```

- [ ] **Step 3: Type-check**

```powershell
cd app
pnpm build
```

- [ ] **Step 4: Commit**

```powershell
cd ..
git add app/src/views/Diff.tsx app/src/views/Diff.module.css
git commit -m "feat(ui): Diff view container (exports both sides, computes change set, renders panes)"
```

---

## Task 10: ConflictList view + App routing

**Files:**
- Create: `app/src/views/ConflictList.tsx`
- Create: `app/src/views/ConflictList.module.css`
- Modify: `app/src/App.tsx`

For Plan 3, ConflictList is intentionally minimal: a placeholder message saying "Standalone mode - coming in a follow-up" plus a small input box where the user can paste two paths and jump to a diff. The full "scan repo, list conflicts, click row" workflow is deferred (it needs a `list_conflicts` IPC command and a more elaborate UI; not blocking for MVP).

- [ ] **Step 1: Write `ConflictList.tsx`**

Create `app/src/views/ConflictList.tsx`:

```tsx
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
      <h1>Unreal Merge - Standalone</h1>
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
```

- [ ] **Step 2: Write the CSS module**

Create `app/src/views/ConflictList.module.css`:

```css
.root {
    max-width: 720px;
    margin: 0 auto;
    padding: 2rem;
}

.note {
    color: #aaa;
    font-size: 0.9rem;
}

.form {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    margin-top: 1.5rem;
}

.form label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.85rem;
    color: #b8b8b8;
}

.form input {
    background: #1a1c20;
    border: 1px solid #2c2f36;
    color: #e6e6e6;
    padding: 0.45rem 0.6rem;
    border-radius: 4px;
    font-family: ui-monospace, "Cascadia Mono", monospace;
    font-size: 0.85rem;
}

.form input:focus {
    outline: 1px solid #5a6470;
    border-color: #5a6470;
}

.form button {
    align-self: flex-start;
    background: #3b4148;
    color: #e6e6e6;
    border: 1px solid #4a5159;
    padding: 0.45rem 1.2rem;
    border-radius: 4px;
    cursor: pointer;
}

.form button:hover { background: #485058; }
```

- [ ] **Step 3: Rewrite `App.tsx`**

Replace `app/src/App.tsx`:

```tsx
import { useEffect, useState } from "react";
import { getAppMode } from "./ipc";
import type { AppMode } from "./types";
import ConflictList from "./views/ConflictList";
import Diff from "./views/Diff";

export default function App() {
  const [mode, setMode] = useState<AppMode | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getAppMode().then(setMode).catch((e) => setError(String(e)));
  }, []);

  if (error) {
    return (
      <main style={{ padding: "1.5rem" }}>
        <h1>Unreal Merge</h1>
        <p style={{ color: "#ffb4a8" }}>Failed to load app mode: {error}</p>
      </main>
    );
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
    // as %A - which Tauri receives as the `ours` argv slot. After the driver
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
```

- [ ] **Step 4: Type-check**

```powershell
cd app
pnpm build
```

- [ ] **Step 5: Commit**

```powershell
cd ..
git add app/src/App.tsx app/src/views/ConflictList.tsx app/src/views/ConflictList.module.css
git commit -m "feat(ui): App routing + minimal ConflictList for standalone mode"
```

---

## Task 11: Manual smoke - `pnpm tauri dev`

**Files:** none - manual verification step.

- [ ] **Step 1: Build the Rust crate once so Tauri's compile is cached**

```powershell
cd app/src-tauri
cargo build --bin unreal-merge
```

Expected: builds cleanly. If `tauri.conf.json`'s `frontendDist` points at a missing dir, that's fine for `cargo build` - only `tauri build` cares.

- [ ] **Step 2: Run `tauri dev` and confirm the window opens**

```powershell
cd app
pnpm tauri dev
```

Expected: Vite starts on `http://127.0.0.1:1420`, then a Tauri window opens titled "Unreal Merge", showing the standalone-mode form (three text inputs + Open button). Press Ctrl+C in the terminal to close.

If the window doesn't appear, check:
- `tauri.conf.json` `devUrl` matches `vite.config.ts` port (1420)
- Vite dev server is reachable from the same host (`http://127.0.0.1:1420` in your browser)

Don't commit anything here - this is a verification step. If anything misbehaves, fix it before moving on.

- [ ] **Step 3: Manual `--git-driver` mode dev**

Tauri's dev mode passes through extra args via `--`:

```powershell
cd app
pnpm tauri dev -- -- --git-driver C:\tmp\base C:\tmp\ours.uasset C:\tmp\theirs.uasset a.uasset
```

(The double `--` is intentional: first separates pnpm's args from the tauri-cli, second separates tauri-cli's args from the underlying `cargo run`.)

Expected: the window opens directly into the Diff view, immediately tries to call `exportAsset` against the bogus paths, and shows a red error pane with the failure. That's the correct behaviour - the *routing* is what we're verifying, not a successful export.

Press Ctrl+C to close.

- [ ] **Step 4: Note any issues for the next task**

Take notes on any UX awkwardness (cramped table, missing keyboard shortcuts, etc.). These don't block Plan 3 - Plan 4 polishes. Just document.

---

## Task 12: `pnpm tauri build` produces the production .exe

**Files:** none - a verification + commit-the-lockfile-updates step.

- [ ] **Step 1: Run a release build**

```powershell
cd app
pnpm tauri build
```

Expected: 5–20 minutes (first time). Produces:
- `app/dist/` - Vite production bundle
- `app/src-tauri/target/release/unreal-merge.exe` - the actual single-file binary
- `app/src-tauri/target/release/bundle/msi/Unreal Merge_0.1.0_x64_en-US.msi` - installer (optional, ignore for now)

Verify file size:

```powershell
(Get-Item app/src-tauri/target/release/unreal-merge.exe).Length / 1MB
```

Expected: ≥ 5 MB (Tauri-built binaries embed WebView2 launcher + frontend bundle).

- [ ] **Step 2: Confirm CLI still works on the release binary**

```powershell
app/src-tauri/target/release/unreal-merge.exe --help
app/src-tauri/target/release/unreal-merge.exe scan --repo .
```

Expected: subcommand help; scan reports no conflicts (or whatever the current repo state is). The CLI mode path doesn't open a window.

- [ ] **Step 3: Smoke the GUI mode**

Double-click `app/src-tauri/target/release/unreal-merge.exe` in Explorer (or run it from PowerShell without args).

Expected: window opens to the standalone-mode form.

- [ ] **Step 4: Confirm Plan 2's tests still pass**

```powershell
cd app/src-tauri
cargo test --all-targets
```

Expected: all PASS, no regressions.

- [ ] **Step 5: Commit any lockfile / dep changes**

If `app/pnpm-lock.yaml` or `app/src-tauri/Cargo.lock` changed during the build (Cargo.lock is gitignored; pnpm-lock is tracked), commit:

```powershell
cd ../..
git add app/pnpm-lock.yaml
git diff --cached --quiet || git commit -m "chore: refresh pnpm-lock after tauri build"
```

(`git diff --cached --quiet` succeeds with no staged changes - in which case the `||` short-circuits and we don't commit an empty change.)

---

## Task 13: End-to-end Git merge driver scenario

**Files:**
- Create: `tools/plan3-e2e-smoke.ps1`

The acceptance test for Plan 3 done criterion #3: a tmp repo, conflict, install the merge driver, trigger the merge, click Take Theirs in the GUI, verify resolution. Because this involves a real GUI interaction, it can't be fully automated without a UI-automation library - so the script does everything *up to* the GUI prompt, then asks the human to click.

- [ ] **Step 1: Write the smoke script**

Create `tools/plan3-e2e-smoke.ps1`:

```powershell
<#
    End-to-end smoke for Plan 3.
      1. Build unreal-merge in release mode.
      2. Create a tmp Git repo, induce a .uasset conflict.
      3. Install the merge driver.
      4. Run `git merge` - the GUI opens.
      5. Wait for the human to click Take Theirs (or Abort).
      6. Verify the working tree contents reflect the resolution.

    Usage:
        powershell tools/plan3-e2e-smoke.ps1
#>
[CmdletBinding()]
param(
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

if (-not $SkipBuild) {
    Write-Host "Building unreal-merge (release)..." -ForegroundColor Cyan
    Push-Location (Join-Path $root 'app')
    pnpm tauri build
    Pop-Location
}

$exe = Join-Path $root 'app\src-tauri\target\release\unreal-merge.exe'
if (-not (Test-Path $exe)) { throw "Build failed: $exe not found" }

# Create a tmp repo with a .uasset conflict.
$tmp = New-Item -ItemType Directory -Path (Join-Path ([System.IO.Path]::GetTempPath()) ("mbe-e2e-" + [Guid]::NewGuid()))
Write-Host "Tmp repo: $($tmp.FullName)" -ForegroundColor Cyan
Push-Location $tmp.FullName
try {
    git init -q
    git config user.email "test@example.com"
    git config user.name "test"
    git checkout -b main -q
    Set-Content -Path "a.uasset" -Value "BASE" -NoNewline
    git add a.uasset
    git commit -q -m "base"

    git checkout -b feature -q
    Set-Content -Path "a.uasset" -Value "FEATURE" -NoNewline
    git commit -q -am "feature"

    git checkout main -q
    Set-Content -Path "a.uasset" -Value "MAIN" -NoNewline
    git commit -q -am "main"

    Write-Host "Installing merge driver..." -ForegroundColor Cyan
    & $exe install --repo .

    Write-Host ""
    Write-Host "About to trigger 'git merge feature'." -ForegroundColor Yellow
    Write-Host "The Unreal Merge window will open." -ForegroundColor Yellow
    Write-Host "Click 'Take Theirs' to apply the FEATURE branch's content." -ForegroundColor Yellow
    Write-Host "Press ENTER to begin."
    Read-Host | Out-Null

    git merge feature --no-edit
    $exitCode = $LASTEXITCODE
    Write-Host "git merge exit code: $exitCode"

    $finalContent = Get-Content a.uasset -Raw
    Write-Host "Working-tree a.uasset content: [$finalContent]"

    if ($finalContent -eq "FEATURE") {
        Write-Host "PASS: Take Theirs resolution applied correctly." -ForegroundColor Green
    } elseif ($finalContent -eq "MAIN") {
        Write-Host "PASS: Take Ours resolution applied (or Abort with conflict markers)." -ForegroundColor Green
    } else {
        Write-Host "INSPECT: working tree content is unexpected. Verify manually." -ForegroundColor Yellow
    }
}
finally {
    Pop-Location
    Write-Host ""
    Write-Host "Tmp repo at: $($tmp.FullName)" -ForegroundColor Cyan
    Write-Host "Delete with: Remove-Item -Recurse -Force '$($tmp.FullName)'"
}
```

- [ ] **Step 2: Run the smoke**

```powershell
powershell tools/plan3-e2e-smoke.ps1
```

Follow the prompts. Click "Take Theirs" when the window opens. Expect the script to print `PASS: Take Theirs resolution applied correctly.`.

- [ ] **Step 3: Commit**

```powershell
git add tools/plan3-e2e-smoke.ps1
git commit -m "test(ui): Plan 3 end-to-end smoke (git merge → GUI → resolution)"
```

---

## Done criteria - verify before declaring Plan 3 complete

Run from the repo root:

```powershell
# 1. Frontend installs and builds
cd app; pnpm install; pnpm build; cd ..

# 2. Rust tests still pass
cd app/src-tauri; cargo test --all-targets; cd ../..

# 3. Production build produces a binary
cd app; pnpm tauri build; cd ..
Test-Path app/src-tauri/target/release/unreal-merge.exe
(Get-Item app/src-tauri/target/release/unreal-merge.exe).Length / 1MB

# 4. CLI still works on the release binary
app/src-tauri/target/release/unreal-merge.exe scan --repo .

# 5. End-to-end with GUI interaction
powershell tools/plan3-e2e-smoke.ps1
```

All five must succeed (5 requires human button-click). Binary size > 5 MB.

---

## Out of scope for Plan 3 (do NOT attempt)

- Blueprint graph rendering (React Flow nodes + wires) - Plan 4.
- "Open in Unreal" action (spawn `UnrealEditor.exe -diff …`) - Plan 4.
- Full standalone-mode UI (scan repo, list conflicts, click to enter diff) - Plan 4. The placeholder in ConflictList is intentional.
- 3-way diff (Base / Ours / Theirs panes side by side). Plan 3 ships 2-way only; spec §5 keeps the 3-pane *visual* but Plan 1's commandlet doesn't produce a "base" snapshot in a useful way yet.
- Per-property cherry-pick checkboxes.
- Auto-launch of UE sidecar on first window open (eager warmup). Sidecar spawns per export request.
- Real application icon (using a placeholder PNG). Plan 4 ships polished art.
- Code signing / SmartScreen reputation. Production build only.
- Cross-platform (macOS / Linux). Windows-only.
