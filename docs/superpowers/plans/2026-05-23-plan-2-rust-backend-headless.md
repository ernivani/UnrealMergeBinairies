# Plan 2 — Rust Backend + Git Merge Driver (Headless)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a single `unreal-merge.exe` Rust binary that, when invoked by Git as a merge driver on a `.uasset` conflict, spawns UE 5.6, exports the three sides of the conflict via Plan 1's commandlet, computes a property-level diff, and resolves the working tree according to `UNREAL_MERGE_RESOLUTION=ours|theirs|abort` (default abort). Also ships `install`, `scan`, and `export` sub-commands so Plan 3's GUI has clean primitives to call.

**Architecture:** A plain Rust crate at `app/src-tauri/` (Tauri scaffolding is added in Plan 3; for now it's just a binary). `sidecar.rs` spawns `UnrealEditor.exe`, writes a UTF-8 stdin payload and reads stdout, extracts balanced JSON objects with a brace-counter (matches `tools/run-commandlet.ps1`'s logic). `diff.rs` is pure functions over the schema types from Plan 1. `git.rs` shells out to `git` for `ls-files -u`, `show :N:path`, and `cat-file blob`. `installer.rs` rewrites `.gitattributes` and `.git/config`. `cli.rs` dispatches the sub-commands.

**Tech Stack:**
- Rust 1.75+ (`rustup default stable`)
- Crates: `serde`, `serde_json`, `clap` (derive), `anyhow`, `walkdir`, `tempfile` (dev)
- Test crates: `assert_cmd`, `predicates`, `pretty_assertions`
- Mock sidecar for tests: a tiny Rust binary in the same crate (`mock_ue_sidecar`) that speaks the same JSON-RPC protocol — keeps tests UE-free.

**Prerequisites:**
- Rust toolchain: `winget install Rustlang.Rustup; rustup default stable`. MSVC toolchain (the one we already use for UE).
- The commandlet from Plan 1 must build cleanly (`tools/run-commandlet.ps1 -StdinText '{"id":1,"cmd":"ping"}'` returns `{"id":1,"ok":true,"pong":"MergeBinariesExport"}`).

**Done criteria** (run all from repo root, all must succeed):
1. `cargo test --manifest-path app/src-tauri/Cargo.toml --all-targets` exits 0.
2. `cargo run --manifest-path app/src-tauri/Cargo.toml --bin unreal-merge -- export Examples/v1/BP_MinimalChar.uasset` prints valid JSON to stdout matching the schema in Plan 1 §6.
3. `cargo run ... -- diff Examples/v1/BP_MinimalChar.uasset Examples/v2/BP_MinimalChar.uasset` exits 0 and prints a structured diff summary (saved-hash differs; properties identical at this depth, as documented in Plan 1's done report).
4. End-to-end scripted scenario in Task 9 PASSes: a tmp git repo with a conflict gets resolved by `unreal-merge --git-driver` with `UNREAL_MERGE_RESOLUTION=theirs`, and `git status` shows the conflict resolved.

---

## File structure for this plan

```
app/
└── src-tauri/
    ├── Cargo.toml
    ├── rustfmt.toml
    ├── src/
    │   ├── main.rs              # entry, dispatches to cli::run()
    │   ├── cli.rs               # clap definition + dispatch
    │   ├── schema.rs            # serde types matching Plan 1's JSON output
    │   ├── sidecar.rs           # spawn UE + JSON-RPC + brace-counter extractor
    │   ├── diff.rs              # AssetSnapshot diff
    │   ├── merge.rs             # apply_resolution(Ours|Theirs|Abort) -> working tree
    │   ├── git.rs               # ls-files -u, show :N:path, cat-file blob
    │   └── installer.rs         # write/revert .gitattributes + .git/config
    └── tests/
        ├── schema_test.rs       # round-trip Plan 1 goldens
        ├── diff_test.rs         # fixture-driven property diff
        ├── sidecar_mock_test.rs # uses mock_ue_sidecar bin
        └── git_driver_test.rs   # tmp repo + mock sidecar end-to-end
```

The `mock_ue_sidecar` is a second `[[bin]]` declared in the same `Cargo.toml`. It reads JSON lines from stdin and emits canned responses — letting all tests run without UnrealEditor.exe.

Each file has one responsibility:
- **`schema.rs`** — pure types (`Package`, `Asset`, `Property`, `PropertyValue`, `AssetSnapshot`). No logic.
- **`sidecar.rs`** — process spawn, stdin write, stdout read, brace-extraction. Knows nothing about asset semantics.
- **`diff.rs`** — pure functions: `diff_snapshots(base, ours, theirs) -> Diff`. No I/O.
- **`merge.rs`** — applies a `Resolution` to the working tree. Knows about file I/O.
- **`git.rs`** — shells out to `git`. Each function is one Git command.
- **`installer.rs`** — installer/uninstaller for the merge driver. Idempotent.
- **`cli.rs`** — clap parsing + top-level dispatch. No business logic.

---

## Task 0: Cargo crate scaffolding

**Files:**
- Create: `app/src-tauri/Cargo.toml`
- Create: `app/src-tauri/rustfmt.toml`
- Create: `app/src-tauri/src/main.rs`
- Create: `app/src-tauri/src/lib.rs`
- Modify: `.gitignore`

- [ ] **Step 1: Write `Cargo.toml`**

Create `app/src-tauri/Cargo.toml`:

```toml
[package]
name = "unreal-merge"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
publish = false

[lib]
name = "unreal_merge"
path = "src/lib.rs"

[[bin]]
name = "unreal-merge"
path = "src/main.rs"

[[bin]]
name = "mock_ue_sidecar"
path = "src/bin/mock_ue_sidecar.rs"
required-features = []

[dependencies]
anyhow = "1.0"
clap = { version = "4.5", features = ["derive", "env"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
walkdir = "2.5"

[dev-dependencies]
assert_cmd = "2.0"
predicates = "3.1"
pretty_assertions = "1.4"
tempfile = "3.10"

[profile.dev]
debug = 1
incremental = true

[profile.release]
lto = "thin"
codegen-units = 1
```

- [ ] **Step 2: Write `rustfmt.toml`**

Create `app/src-tauri/rustfmt.toml`:

```toml
edition = "2021"
max_width = 100
hard_tabs = false
tab_spaces = 4
use_field_init_shorthand = true
imports_granularity = "Module"
```

- [ ] **Step 3: Write minimal `lib.rs` and `main.rs`**

Create `app/src-tauri/src/lib.rs`:

```rust
//! Backend for unreal-merge: spawn UE commandlet, diff snapshots, resolve conflicts.

pub mod schema;
```

Create `app/src-tauri/src/main.rs`:

```rust
fn main() -> anyhow::Result<()> {
    eprintln!("unreal-merge v0.1.0 (scaffold)");
    Ok(())
}
```

Create the `bin/` subdirectory and a placeholder for the mock sidecar so `cargo build --all-targets` succeeds:

`app/src-tauri/src/bin/mock_ue_sidecar.rs`:

```rust
fn main() {
    eprintln!("mock_ue_sidecar v0.1.0 (placeholder)");
}
```

Create `app/src-tauri/src/schema.rs` as an empty file (Task 1 populates it):

```rust
// Populated in Task 1.
```

- [ ] **Step 4: Update `.gitignore`**

Append to `.gitignore`:

```gitignore
# Rust build artefacts
app/src-tauri/target/
app/src-tauri/Cargo.lock
```

(We intentionally ignore `Cargo.lock` because this is a binary that's not yet released; once Plan 3 ships GUI binaries we'll commit the lock for reproducible builds.)

- [ ] **Step 5: Build and confirm the scaffold compiles**

Run:

```bash
cd app/src-tauri && cargo build --all-targets
```

Expected: builds the `unreal-merge` and `mock_ue_sidecar` binaries with zero errors and zero warnings.

- [ ] **Step 6: Commit**

```bash
git add app/src-tauri .gitignore
git commit -m "feat(rust): scaffold unreal-merge crate"
```

---

## Task 1: Schema types matching Plan 1's JSON output

**Files:**
- Modify: `app/src-tauri/src/schema.rs`
- Create: `app/src-tauri/tests/schema_test.rs`

We deserialize Plan 1's commandlet output into strongly-typed Rust structs. The schema is locked by `Examples/v1.expected.json` and `Examples/v2.expected.json` — those serve as the ground truth.

- [ ] **Step 1: Write the failing round-trip test**

Create `app/src-tauri/tests/schema_test.rs`:

```rust
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use unreal_merge::schema::AssetSnapshot;

fn repo_root() -> PathBuf {
    // workspace root is two levels up from CARGO_MANIFEST_DIR (app/src-tauri/)
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn parse_v1_golden() {
    let path = repo_root().join("Examples").join("v1.expected.json");
    let raw = std::fs::read_to_string(&path).expect("read v1 golden");
    let snap: AssetSnapshot = serde_json::from_str(&raw).expect("parse v1 golden");

    assert_eq!(snap.ok, true);
    assert_eq!(snap.package.name, "/MergeTmp/BP_MinimalChar");
    assert_eq!(snap.package.file_version_ue5, 1017);
    assert!(snap.package.saved_hash.starts_with("sha1:"));
    assert_eq!(snap.asset.class, "Blueprint");
    assert!(snap.asset.properties.len() > 30);
}

#[test]
fn v1_and_v2_have_same_property_count() {
    let v1: AssetSnapshot = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("Examples/v1.expected.json")).unwrap(),
    )
    .unwrap();
    let v2: AssetSnapshot = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("Examples/v2.expected.json")).unwrap(),
    )
    .unwrap();
    // Plan 1's done report: at this schema depth v1 and v2 are byte-identical
    // except savedHash. Lock this expectation so a future regression that
    // accidentally walks deeper into structs is flagged immediately.
    assert_eq!(v1.asset.properties.len(), v2.asset.properties.len());
    assert_ne!(v1.package.saved_hash, v2.package.saved_hash);
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd app/src-tauri && cargo test --test schema_test
```

Expected: compile error — `unreal_merge::schema::AssetSnapshot` doesn't exist.

- [ ] **Step 3: Implement the schema types**

Replace `app/src-tauri/src/schema.rs` with:

```rust
//! Wire types for the JSON emitted by ue-host/Plugins/MergeBinariesExport.
//!
//! These deserialise the response shape from Plan 1 §6. Field naming follows
//! the JSON (camelCase) via serde rename, while Rust field names stay snake.

use serde::{Deserialize, Serialize};

/// A full export response from the commandlet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSnapshot {
    /// Echo of the request id (only present when the request had one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,

    pub ok: bool,

    /// Echo of the input path (absolute, OS-shaped). Goldens strip this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    pub package: Package,

    pub asset: Asset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,

    #[serde(rename = "engineVersion")]
    pub engine_version: String,

    #[serde(rename = "fileVersionUE5")]
    pub file_version_ue5: u32,

    #[serde(rename = "savedHash")]
    pub saved_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub class: String,

    #[serde(rename = "parentClass", default)]
    pub parent_class: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub properties: Vec<Property>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Property {
    pub path: String,

    #[serde(rename = "type")]
    pub ty: String,

    pub value: PropertyValue,
}

/// Property values are dynamic — they can be a primitive (bool/number/string)
/// or a typed-summary object for structs/arrays/maps/sets. We accept any JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PropertyValue {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Summary(serde_json::Map<String, serde_json::Value>),
}

/// Wire-format response when the commandlet reports an error (`ok:false`).
/// We don't deserialise into AssetSnapshot in that case — call sites should
/// branch on `ok` before treating a response as a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    pub ok: bool, // always false for this variant
    pub error: String,
}
```

Also re-export from `lib.rs`:

Replace `app/src-tauri/src/lib.rs` with:

```rust
//! Backend for unreal-merge: spawn UE commandlet, diff snapshots, resolve conflicts.

pub mod schema;

pub use schema::{Asset, AssetSnapshot, ErrorResponse, Package, Property, PropertyValue};
```

- [ ] **Step 4: Run the test to verify it passes**

```bash
cd app/src-tauri && cargo test --test schema_test
```

Expected: both tests PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri
git commit -m "feat(rust): schema types + round-trip against Plan 1 goldens"
```

---

## Task 2: Property diff

**Files:**
- Create: `app/src-tauri/src/diff.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Create: `app/src-tauri/tests/diff_test.rs`

A 3-way diff is the ultimate goal, but we start with a 2-way diff (ours vs theirs) which is sufficient for the merge UI: it shows what changed between the two sides without needing the base for context.

- [ ] **Step 1: Write the failing diff test**

Create `app/src-tauri/tests/diff_test.rs`:

```rust
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use unreal_merge::diff::{PropertyChange, diff_properties};
use unreal_merge::schema::AssetSnapshot;

fn load_snapshot(name: &str) -> AssetSnapshot {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let path = root.join("Examples").join(name);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

#[test]
fn v1_vs_v2_no_property_diffs_at_current_depth() {
    let v1 = load_snapshot("v1.expected.json");
    let v2 = load_snapshot("v2.expected.json");
    let diffs = diff_properties(&v1.asset.properties, &v2.asset.properties);
    // Plan 1's done report: shallow walk produces identical property arrays.
    assert!(diffs.is_empty(), "Expected no property diffs, got: {:#?}", diffs);
}

#[test]
fn detects_added_property() {
    use unreal_merge::schema::{Property, PropertyValue};
    let base: Vec<Property> = vec![Property {
        path: "FieldA".to_string(),
        ty: "bool".to_string(),
        value: PropertyValue::Bool(true),
    }];
    let mut other = base.clone();
    other.push(Property {
        path: "FieldB".to_string(),
        ty: "int32".to_string(),
        value: PropertyValue::Number(7.into()),
    });
    let diffs = diff_properties(&base, &other);
    assert_eq!(diffs.len(), 1);
    match &diffs[0] {
        PropertyChange::Added { path, .. } => assert_eq!(path, "FieldB"),
        other => panic!("expected Added, got {:?}", other),
    }
}

#[test]
fn detects_removed_property() {
    use unreal_merge::schema::{Property, PropertyValue};
    let base = vec![
        Property {
            path: "FieldA".to_string(),
            ty: "bool".to_string(),
            value: PropertyValue::Bool(true),
        },
        Property {
            path: "FieldB".to_string(),
            ty: "int32".to_string(),
            value: PropertyValue::Number(7.into()),
        },
    ];
    let other = base[..1].to_vec();
    let diffs = diff_properties(&base, &other);
    assert_eq!(diffs.len(), 1);
    match &diffs[0] {
        PropertyChange::Removed { path, .. } => assert_eq!(path, "FieldB"),
        other => panic!("expected Removed, got {:?}", other),
    }
}

#[test]
fn detects_changed_property() {
    use unreal_merge::schema::{Property, PropertyValue};
    let base = vec![Property {
        path: "Speed".to_string(),
        ty: "float".to_string(),
        value: PropertyValue::Number(serde_json::Number::from_f64(600.0).unwrap()),
    }];
    let mut other = base.clone();
    other[0].value = PropertyValue::Number(serde_json::Number::from_f64(750.0).unwrap());
    let diffs = diff_properties(&base, &other);
    assert_eq!(diffs.len(), 1);
    match &diffs[0] {
        PropertyChange::Changed { path, .. } => assert_eq!(path, "Speed"),
        other => panic!("expected Changed, got {:?}", other),
    }
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd app/src-tauri && cargo test --test diff_test
```

Expected: compile error — `diff` module not found.

- [ ] **Step 3: Implement `diff.rs`**

Create `app/src-tauri/src/diff.rs`:

```rust
//! Pure diff functions over `Property` lists. Order-independent; keyed by `path`.

use crate::schema::{Property, PropertyValue};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyChange {
    Added {
        path: String,
        ty: String,
        value: PropertyValue,
    },
    Removed {
        path: String,
        ty: String,
        value: PropertyValue,
    },
    Changed {
        path: String,
        ty: String,
        old: PropertyValue,
        new: PropertyValue,
    },
}

/// 2-way diff: what changed going from `base` to `other`. Symmetric (swap args to flip).
pub fn diff_properties(base: &[Property], other: &[Property]) -> Vec<PropertyChange> {
    let base_map: HashMap<&str, &Property> = base.iter().map(|p| (p.path.as_str(), p)).collect();
    let other_map: HashMap<&str, &Property> = other.iter().map(|p| (p.path.as_str(), p)).collect();

    let mut changes = Vec::new();

    // Pass 1: things present in `base`; check if other has them.
    for (path, base_prop) in &base_map {
        match other_map.get(path) {
            None => changes.push(PropertyChange::Removed {
                path: path.to_string(),
                ty: base_prop.ty.clone(),
                value: base_prop.value.clone(),
            }),
            Some(other_prop) if other_prop.value != base_prop.value => {
                changes.push(PropertyChange::Changed {
                    path: path.to_string(),
                    ty: base_prop.ty.clone(),
                    old: base_prop.value.clone(),
                    new: other_prop.value.clone(),
                })
            }
            Some(_) => {}
        }
    }

    // Pass 2: things added in `other` that didn't exist in `base`.
    for (path, other_prop) in &other_map {
        if !base_map.contains_key(path) {
            changes.push(PropertyChange::Added {
                path: path.to_string(),
                ty: other_prop.ty.clone(),
                value: other_prop.value.clone(),
            });
        }
    }

    // Sort by path so output is deterministic.
    changes.sort_by(|a, b| change_path(a).cmp(change_path(b)));
    changes
}

fn change_path(c: &PropertyChange) -> &str {
    match c {
        PropertyChange::Added { path, .. }
        | PropertyChange::Removed { path, .. }
        | PropertyChange::Changed { path, .. } => path,
    }
}
```

Update `app/src-tauri/src/lib.rs`:

```rust
//! Backend for unreal-merge: spawn UE commandlet, diff snapshots, resolve conflicts.

pub mod diff;
pub mod schema;

pub use diff::{PropertyChange, diff_properties};
pub use schema::{Asset, AssetSnapshot, ErrorResponse, Package, Property, PropertyValue};
```

- [ ] **Step 4: Run to verify all 4 tests pass**

```bash
cd app/src-tauri && cargo test --test diff_test
```

Expected: 4 passed; 0 failed.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri
git commit -m "feat(rust): 2-way property diff with Add/Remove/Change variants"
```

---

## Task 3: Mock UE sidecar binary

**Files:**
- Modify: `app/src-tauri/src/bin/mock_ue_sidecar.rs`
- Create: `app/src-tauri/tests/mock_sidecar_test.rs`

The mock binary is what all sidecar tests target. It implements the same JSON-RPC framing as Plan 1's commandlet, but returns canned data so tests don't need UnrealEditor.exe. It deliberately mimics one quirk: it emits an unrelated log line on stdout before its first response, so the brace-counter logic gets exercised.

- [ ] **Step 1: Write the failing integration test**

Create `app/src-tauri/tests/mock_sidecar_test.rs`:

```rust
use assert_cmd::Command;
use pretty_assertions::assert_eq;
use std::io::Write;

#[test]
fn mock_handles_ping_and_quit() {
    let mut cmd = Command::cargo_bin("mock_ue_sidecar").unwrap();
    cmd.write_stdin(
        "{\"id\":1,\"cmd\":\"ping\"}\n{\"id\":2,\"cmd\":\"quit\"}\n",
    );
    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();

    // Expect: noise line then two JSON responses.
    let json_lines: Vec<&str> = stdout
        .lines()
        .filter(|l| l.starts_with('{'))
        .collect();
    assert_eq!(json_lines.len(), 2);
    assert!(json_lines[0].contains("\"pong\":\"mock_ue_sidecar\""));
    assert!(json_lines[1].contains("\"id\":2"));
}

#[test]
fn mock_export_returns_canned_snapshot() {
    let mut cmd = Command::cargo_bin("mock_ue_sidecar").unwrap();
    cmd.write_stdin(
        "{\"id\":1,\"cmd\":\"export\",\"path\":\"X:/anything.uasset\"}\n{\"id\":2,\"cmd\":\"quit\"}\n",
    );
    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();

    let resp: serde_json::Value = stdout
        .lines()
        .filter(|l| l.starts_with('{'))
        .find_map(|l| serde_json::from_str(l).ok())
        .expect("at least one JSON response");
    assert_eq!(resp["ok"], true);
    assert_eq!(resp["asset"]["class"], "Blueprint");
    assert_eq!(resp["package"]["fileVersionUE5"], 1017);
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd app/src-tauri && cargo test --test mock_sidecar_test
```

Expected: tests build but FAIL — current mock binary just prints a banner.

- [ ] **Step 3: Implement the mock**

Replace `app/src-tauri/src/bin/mock_ue_sidecar.rs`:

```rust
//! Test double for the UE 5.6 MergeBinariesExport commandlet.
//!
//! Speaks the same JSON-RPC framing (newline-delimited JSON over stdio), supports
//! the same set of cmds (`ping`, `export`, `quit`), and emits a couple of fake log
//! lines on stdout before its first response — so consumers' brace-counter
//! extractors are exercised against realistic noise.

use std::io::{self, BufRead, Write};

fn write_json(value: &serde_json::Value) {
    let mut out = io::stdout().lock();
    let s = serde_json::to_string(value).unwrap();
    writeln!(out, "{}", s).unwrap();
    out.flush().unwrap();
}

fn emit_fake_log() {
    let mut out = io::stdout().lock();
    writeln!(
        out,
        "[2026.05.23-12.34.56:789][  0]LogStreaming: Display: this is mock noise"
    )
    .unwrap();
    out.flush().unwrap();
}

fn handle_export(path: &str, id: Option<&serde_json::Value>) -> serde_json::Value {
    let mut resp = serde_json::json!({
        "ok": true,
        "path": path,
        "package": {
            "name": "/MergeTmp/MockAsset",
            "engineVersion": "5.6.0-mock+++UE5+Release-5.6",
            "fileVersionUE5": 1017,
            "savedHash": "sha1:0000000000000000000000000000000000000000"
        },
        "asset": {
            "class": "Blueprint",
            "parentClass": "/Script/Engine.BlueprintCore",
            "name": "MockAsset",
            "properties": [
                {"path": "bMockBool", "type": "bool", "value": false},
                {"path": "MockString", "type": "FString", "value": "hello"}
            ]
        }
    });
    if let Some(id_val) = id {
        resp["id"] = id_val.clone();
    }
    resp
}

fn main() {
    let stdin = io::stdin();
    let mut first = true;
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            _ => continue,
        };
        if first {
            emit_fake_log();
            first = false;
        }
        let req: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                write_json(&serde_json::json!({"ok": false, "error": "invalid JSON on stdin"}));
                continue;
            }
        };
        let id = req.get("id").cloned();
        match req.get("cmd").and_then(|c| c.as_str()) {
            Some("ping") => {
                let mut resp = serde_json::json!({"ok": true, "pong": "mock_ue_sidecar"});
                if let Some(v) = &id {
                    resp["id"] = v.clone();
                }
                write_json(&resp);
            }
            Some("export") => {
                let path = req
                    .get("path")
                    .and_then(|p| p.as_str())
                    .unwrap_or("");
                write_json(&handle_export(path, id.as_ref()));
            }
            Some("quit") => {
                let mut resp = serde_json::json!({"ok": true});
                if let Some(v) = &id {
                    resp["id"] = v.clone();
                }
                write_json(&resp);
                return;
            }
            _ => {
                let mut resp = serde_json::json!({"ok": false, "error": "unknown cmd"});
                if let Some(v) = &id {
                    resp["id"] = v.clone();
                }
                write_json(&resp);
            }
        }
    }
}
```

The mock uses `serde_json::Value` directly (no schema types) so it stays independent of the production schema — that way schema regressions don't accidentally hide bugs in the production parser.

- [ ] **Step 4: Run to verify it passes**

```bash
cd app/src-tauri && cargo test --test mock_sidecar_test
```

Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri
git commit -m "feat(rust): mock UE sidecar binary for offline testing"
```

---

## Task 4: Real sidecar (process spawn + JSON extraction)

**Files:**
- Create: `app/src-tauri/src/sidecar.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Create: `app/src-tauri/tests/sidecar_test.rs`

This is the heart of Plan 2: the Rust replacement for the launcher logic in `tools/run-commandlet.ps1`. It spawns a child process (UE or the mock), sends a UTF-8 stdin payload (with the warmup prepend), reads stdout into one string, runs the brace-counter to extract JSON objects, and returns them as parsed responses keyed by `id`.

- [ ] **Step 1: Write the failing test**

Create `app/src-tauri/tests/sidecar_test.rs`:

```rust
use std::collections::HashMap;
use unreal_merge::sidecar::{Sidecar, SidecarConfig};

fn mock_path() -> std::path::PathBuf {
    // assert_cmd's helper gives us the path to a binary built by Cargo.
    assert_cmd::cargo::cargo_bin("mock_ue_sidecar")
}

#[test]
fn round_trips_ping_via_mock() {
    let cfg = SidecarConfig {
        executable: mock_path(),
        args: vec![],
        prepend_warmup: true,
        log_redirect: None,
    };
    let sidecar = Sidecar::new(cfg);
    let requests = vec![
        serde_json::json!({"id": 1, "cmd": "ping"}),
        serde_json::json!({"id": 2, "cmd": "quit"}),
    ];
    let responses = sidecar.run_batch(&requests).expect("run batch");
    // Filter to responses with id >= 1 (drop warmup id=0).
    let by_id: HashMap<u64, serde_json::Value> = responses
        .iter()
        .filter_map(|v| v.get("id").and_then(|i| i.as_u64()).map(|id| (id, v.clone())))
        .collect();
    assert_eq!(by_id.len(), 2, "got: {:#?}", responses);
    assert_eq!(by_id[&1]["pong"], "mock_ue_sidecar");
    assert_eq!(by_id[&2]["ok"], true);
}

#[test]
fn export_against_mock_returns_blueprint_snapshot() {
    let cfg = SidecarConfig {
        executable: mock_path(),
        args: vec![],
        prepend_warmup: true,
        log_redirect: None,
    };
    let sidecar = Sidecar::new(cfg);
    let resps = sidecar
        .run_batch(&[
            serde_json::json!({"id": 1, "cmd": "export", "path": "X:/foo.uasset"}),
            serde_json::json!({"id": 2, "cmd": "quit"}),
        ])
        .expect("batch");
    let export = resps
        .iter()
        .find(|r| r.get("id").and_then(|i| i.as_u64()) == Some(1))
        .expect("export response");
    assert_eq!(export["ok"], true);
    assert_eq!(export["asset"]["class"], "Blueprint");
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd app/src-tauri && cargo test --test sidecar_test
```

Expected: compile error — `sidecar` module doesn't exist.

- [ ] **Step 3: Implement the sidecar**

Create `app/src-tauri/src/sidecar.rs`:

```rust
//! Spawns the UE commandlet (or test mock), sends JSON-RPC frames over stdin,
//! and extracts balanced top-level JSON objects from stdout.
//!
//! Mirrors `tools/run-commandlet.ps1`'s logic; the launcher script is kept as
//! the canonical reference and for ad-hoc shell smoke testing.

use anyhow::{Context, Result};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct SidecarConfig {
    /// Path to the executable to spawn (UnrealEditor.exe in production, mock in tests).
    pub executable: PathBuf,
    /// Args passed to the executable BEFORE the JSON-RPC stdio session starts.
    pub args: Vec<String>,
    /// When true, prepend `{"id":0,"cmd":"_warmup"}` to the stdin payload
    /// to absorb UE's first-stdin-line eating. Set false for the mock.
    pub prepend_warmup: bool,
    /// If set, pass `-AbsLog=<path>` to UE so its logs don't interleave with
    /// our JSON frames on stdout. Ignored for the mock.
    pub log_redirect: Option<PathBuf>,
}

pub struct Sidecar {
    cfg: SidecarConfig,
}

impl Sidecar {
    pub fn new(cfg: SidecarConfig) -> Self {
        Self { cfg }
    }

    /// Send a sequence of JSON requests and return the captured JSON responses.
    /// Always appends a `{"cmd":"quit"}` if the caller hasn't already, so the
    /// child exits cleanly.
    pub fn run_batch(&self, requests: &[serde_json::Value]) -> Result<Vec<serde_json::Value>> {
        let mut payload = String::new();
        if self.cfg.prepend_warmup {
            payload.push_str("{\"id\":0,\"cmd\":\"_warmup\"}\n");
        }
        for req in requests {
            payload.push_str(&serde_json::to_string(req)?);
            payload.push('\n');
        }
        if !requests
            .iter()
            .any(|r| r.get("cmd").and_then(|c| c.as_str()) == Some("quit"))
        {
            payload.push_str("{\"cmd\":\"quit\"}\n");
        }

        let mut cmd = Command::new(&self.cfg.executable);
        cmd.args(&self.cfg.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(log) = &self.cfg.log_redirect {
            cmd.arg(format!("-AbsLog={}", log.display()));
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("spawning {}", self.cfg.executable.display()))?;

        // Write stdin in a scope so it drops (and the pipe closes) before we wait.
        {
            let stdin = child
                .stdin
                .as_mut()
                .context("child has no stdin")?;
            stdin
                .write_all(payload.as_bytes())
                .context("writing stdin payload")?;
        }

        let output = child
            .wait_with_output()
            .context("waiting for child")?;
        // We don't check exit code — UE may exit nonzero for incidental reasons
        // even when all our exports succeeded. Trust the in-band JSON instead.

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(extract_json_objects(&stdout))
    }
}

/// Walk a string and return every balanced top-level JSON object found in it,
/// parsed via serde_json. Robust to text noise around or between objects.
pub fn extract_json_objects(text: &str) -> Vec<serde_json::Value> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        // Scan for the matching close brace, tracking string/escape state.
        let mut depth: i32 = 0;
        let mut in_str = false;
        let mut esc = false;
        let start = i;
        let mut end = None;
        let mut j = i;
        while j < bytes.len() {
            let c = bytes[j];
            if in_str {
                if esc {
                    esc = false;
                } else if c == b'\\' {
                    esc = true;
                } else if c == b'"' {
                    in_str = false;
                }
            } else if c == b'"' {
                in_str = true;
            } else if c == b'{' {
                depth += 1;
            } else if c == b'}' {
                depth -= 1;
                if depth == 0 {
                    end = Some(j);
                    break;
                }
            }
            j += 1;
        }
        match end {
            Some(e) => {
                let slice = &text[start..=e];
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(slice) {
                    out.push(val);
                }
                i = e + 1;
            }
            None => break, // unclosed; stop scanning
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_two_objects_with_noise_between() {
        let text = "garbage {\"id\":1} more garbage {\"id\":2,\"x\":\"a }\"} end";
        let objs = extract_json_objects(text);
        assert_eq!(objs.len(), 2);
        assert_eq!(objs[0]["id"], 1);
        assert_eq!(objs[1]["id"], 2);
    }

    #[test]
    fn handles_nested_braces_inside_strings() {
        let text = "noise {\"a\":\"contains } and { inside\",\"b\":3} end";
        let objs = extract_json_objects(text);
        assert_eq!(objs.len(), 1);
        assert_eq!(objs[0]["b"], 3);
    }

    #[test]
    fn handles_escaped_quotes() {
        let text = r#"{"a":"with \"quotes\" inside","b":1}"#;
        let objs = extract_json_objects(text);
        assert_eq!(objs.len(), 1);
        assert_eq!(objs[0]["b"], 1);
    }
}
```

Update `app/src-tauri/src/lib.rs`:

```rust
//! Backend for unreal-merge: spawn UE commandlet, diff snapshots, resolve conflicts.

pub mod diff;
pub mod schema;
pub mod sidecar;

pub use diff::{PropertyChange, diff_properties};
pub use schema::{Asset, AssetSnapshot, ErrorResponse, Package, Property, PropertyValue};
pub use sidecar::{Sidecar, SidecarConfig, extract_json_objects};
```

- [ ] **Step 4: Run all tests**

```bash
cd app/src-tauri && cargo test
```

Expected: schema, diff, mock sidecar, real sidecar, AND inline `extract_json_objects` tests all PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri
git commit -m "feat(rust): sidecar manager + JSON brace extractor"
```

---

## Task 5: Minimal Git operations

**Files:**
- Create: `app/src-tauri/src/git.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Create: `app/src-tauri/tests/git_test.rs`

We only need three operations in Plan 2: enumerate conflicts, read a specific stage of a conflicted file to a temp path, and mark a file as resolved.

- [ ] **Step 1: Write the failing test**

Create `app/src-tauri/tests/git_test.rs`:

```rust
use std::process::Command;
use tempfile::TempDir;
use unreal_merge::git;

/// Create a tmp git repo with a manually-induced binary conflict between branches.
/// Returns (tmpdir, repo path, conflicted file relative path).
fn create_conflict_repo() -> (TempDir, std::path::PathBuf, String) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(&path)
            .status()
            .unwrap();
        assert!(status.success(), "git {:?} failed", args);
    };

    git(&["init", "-q"]);
    git(&["config", "user.email", "test@example.com"]);
    git(&["config", "user.name", "test"]);
    git(&["checkout", "-b", "main", "-q"]);
    std::fs::write(path.join("Asset.uasset"), b"BASE-CONTENT").unwrap();
    git(&["add", "Asset.uasset"]);
    git(&["commit", "-q", "-m", "base"]);

    git(&["checkout", "-b", "feature", "-q"]);
    std::fs::write(path.join("Asset.uasset"), b"FEATURE-CONTENT").unwrap();
    git(&["commit", "-q", "-am", "feature edit"]);

    git(&["checkout", "main", "-q"]);
    std::fs::write(path.join("Asset.uasset"), b"MAIN-CONTENT").unwrap();
    git(&["commit", "-q", "-am", "main edit"]);

    // Attempt merge — should fail because binary, leaving conflict.
    let _ = Command::new("git")
        .args(["merge", "feature", "--no-edit"])
        .current_dir(&path)
        .status();

    (dir, path, "Asset.uasset".to_string())
}

#[test]
fn lists_uasset_conflicts() {
    let (_tmp, repo, _) = create_conflict_repo();
    let conflicts = git::list_conflicts(&repo).unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0], "Asset.uasset");
}

#[test]
fn reads_stages_to_temp_files() {
    let (_tmp, repo, file) = create_conflict_repo();
    let stages = git::read_stages(&repo, &file).unwrap();
    let base = std::fs::read(&stages.base).unwrap();
    let ours = std::fs::read(&stages.ours).unwrap();
    let theirs = std::fs::read(&stages.theirs).unwrap();
    assert_eq!(base, b"BASE-CONTENT");
    assert_eq!(ours, b"MAIN-CONTENT");
    assert_eq!(theirs, b"FEATURE-CONTENT");
}

#[test]
fn mark_resolved_runs_git_add() {
    let (_tmp, repo, file) = create_conflict_repo();
    // Write a resolution overwriting the working file.
    std::fs::write(repo.join(&file), b"RESOLVED").unwrap();
    git::mark_resolved(&repo, &file).unwrap();
    // After this, the file should no longer be in unmerged state.
    let conflicts = git::list_conflicts(&repo).unwrap();
    assert!(conflicts.is_empty());
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd app/src-tauri && cargo test --test git_test
```

Expected: compile error — `git` module doesn't exist.

- [ ] **Step 3: Implement `git.rs`**

Create `app/src-tauri/src/git.rs`:

```rust
//! Thin shell-out helpers over `git`. One responsibility per function.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

/// List paths in unmerged (conflicted) state matching `*.uasset` or `*.umap`.
pub fn list_conflicts(repo: &Path) -> Result<Vec<String>> {
    let out = Command::new("git")
        .args(["ls-files", "-u", "-z"])
        .current_dir(repo)
        .output()
        .context("git ls-files -u")?;
    if !out.status.success() {
        bail!("git ls-files -u failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    // Format: each entry is `<mode> <sha> <stage>\t<path>\0`. Same path appears
    // at stages 1, 2, 3 — we dedupe.
    let text = String::from_utf8_lossy(&out.stdout);
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for entry in text.split('\0').filter(|e| !e.is_empty()) {
        if let Some(idx) = entry.find('\t') {
            let path = &entry[idx + 1..];
            if path.ends_with(".uasset") || path.ends_with(".umap") {
                seen.insert(path.to_string());
            }
        }
    }
    Ok(seen.into_iter().collect())
}

pub struct ConflictStages {
    pub base: PathBuf,
    pub ours: PathBuf,
    pub theirs: PathBuf,
    _tmp: tempfile::TempDir,
}

/// Materialise the three stages of `path` (base=1, ours=2, theirs=3) to temp files.
/// The returned `ConflictStages` owns a `TempDir`; when it drops, the files vanish.
pub fn read_stages(repo: &Path, path: &str) -> Result<ConflictStages> {
    let tmp = tempfile::tempdir().context("create tempdir for stages")?;
    let base = stage_to_path(repo, path, 1, tmp.path(), "base")?;
    let ours = stage_to_path(repo, path, 2, tmp.path(), "ours")?;
    let theirs = stage_to_path(repo, path, 3, tmp.path(), "theirs")?;
    Ok(ConflictStages {
        base,
        ours,
        theirs,
        _tmp: tmp,
    })
}

fn stage_to_path(
    repo: &Path,
    path: &str,
    stage: u8,
    dir: &Path,
    label: &str,
) -> Result<PathBuf> {
    // Preserve the filename so UE's loader sees a reasonable extension.
    let leaf = Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "stage.bin".to_string());
    let out_path = dir.join(format!("{}_{}", label, leaf));
    let spec = format!(":{}:{}", stage, path);
    let out = Command::new("git")
        .args(["show", &spec])
        .current_dir(repo)
        .output()
        .context("git show stage")?;
    if !out.status.success() {
        bail!(
            "git show {} failed: {}",
            spec,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    std::fs::write(&out_path, &out.stdout).context("write stage temp")?;
    Ok(out_path)
}

/// Mark `path` as resolved (`git add`).
pub fn mark_resolved(repo: &Path, path: &str) -> Result<()> {
    let status = Command::new("git")
        .args(["add", "--", path])
        .current_dir(repo)
        .status()
        .context("git add")?;
    if !status.success() {
        bail!("git add {} failed", path);
    }
    Ok(())
}
```

Add `tempfile = "3.10"` to the **regular** dependencies (it was only in dev-dependencies before). Edit the `[dependencies]` section of `app/src-tauri/Cargo.toml`:

```toml
tempfile = "3.10"
```

Update `app/src-tauri/src/lib.rs` to expose the new module:

```rust
pub mod diff;
pub mod git;
pub mod schema;
pub mod sidecar;
```

- [ ] **Step 4: Run all tests**

```bash
cd app/src-tauri && cargo test
```

Expected: all tests including the three new git ones PASS. Note: the git tests SKIP if `git` isn't on PATH — but we know it is in this environment.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri
git commit -m "feat(rust): minimal git ops (list, read stages, mark resolved)"
```

---

## Task 6: Resolution merger

**Files:**
- Create: `app/src-tauri/src/merge.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Create: `app/src-tauri/tests/merge_test.rs`

The merger is what `--git-driver` mode invokes after deciding (or being told) how to resolve: it copies the chosen side over the working tree path, restoring read-only state if the target was LFS-locked (spec §8 case 8a).

- [ ] **Step 1: Write the failing test**

Create `app/src-tauri/tests/merge_test.rs`:

```rust
use tempfile::TempDir;
use unreal_merge::merge::{Resolution, apply_resolution};

#[test]
fn ours_copies_ours_over_dest() {
    let tmp = TempDir::new().unwrap();
    let ours = tmp.path().join("ours.bin");
    let theirs = tmp.path().join("theirs.bin");
    let dest = tmp.path().join("dest.bin");
    std::fs::write(&ours, b"OURS").unwrap();
    std::fs::write(&theirs, b"THEIRS").unwrap();
    std::fs::write(&dest, b"STALE").unwrap();
    apply_resolution(Resolution::Ours, &ours, &theirs, &dest).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"OURS");
}

#[test]
fn theirs_copies_theirs_over_dest() {
    let tmp = TempDir::new().unwrap();
    let ours = tmp.path().join("ours.bin");
    let theirs = tmp.path().join("theirs.bin");
    let dest = tmp.path().join("dest.bin");
    std::fs::write(&ours, b"OURS").unwrap();
    std::fs::write(&theirs, b"THEIRS").unwrap();
    std::fs::write(&dest, b"STALE").unwrap();
    apply_resolution(Resolution::Theirs, &ours, &theirs, &dest).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"THEIRS");
}

#[test]
fn abort_returns_error() {
    let tmp = TempDir::new().unwrap();
    let ours = tmp.path().join("ours.bin");
    let theirs = tmp.path().join("theirs.bin");
    let dest = tmp.path().join("dest.bin");
    std::fs::write(&ours, b"OURS").unwrap();
    std::fs::write(&theirs, b"THEIRS").unwrap();
    std::fs::write(&dest, b"STALE").unwrap();
    let err = apply_resolution(Resolution::Abort, &ours, &theirs, &dest);
    assert!(err.is_err(), "Abort should produce an error");
    assert_eq!(std::fs::read(&dest).unwrap(), b"STALE", "dest must be untouched");
}

#[test]
fn handles_readonly_dest() {
    let tmp = TempDir::new().unwrap();
    let ours = tmp.path().join("ours.bin");
    let theirs = tmp.path().join("theirs.bin");
    let dest = tmp.path().join("dest.bin");
    std::fs::write(&ours, b"OURS").unwrap();
    std::fs::write(&theirs, b"THEIRS").unwrap();
    std::fs::write(&dest, b"STALE").unwrap();
    // Mark dest read-only (simulates LFS lockable).
    let mut perms = std::fs::metadata(&dest).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&dest, perms).unwrap();
    apply_resolution(Resolution::Theirs, &ours, &theirs, &dest).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"THEIRS");
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd app/src-tauri && cargo test --test merge_test
```

Expected: compile error — `merge` module doesn't exist.

- [ ] **Step 3: Implement `merge.rs`**

Create `app/src-tauri/src/merge.rs`:

```rust
//! Apply a Resolution to the working-tree file. Handles read-only LFS-locked
//! files per spec §8 case 8a.

use anyhow::{Result, bail};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    Ours,
    Theirs,
    Abort,
}

impl std::str::FromStr for Resolution {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "ours" => Ok(Self::Ours),
            "theirs" => Ok(Self::Theirs),
            "abort" => Ok(Self::Abort),
            other => bail!("unknown resolution {:?}; expected ours|theirs|abort", other),
        }
    }
}

/// Copy `ours` or `theirs` over `dest`. Returns Err on Abort (deliberately —
/// `--git-driver` mode then exits non-zero, signalling Git to leave the
/// conflict in place).
pub fn apply_resolution(res: Resolution, ours: &Path, theirs: &Path, dest: &Path) -> Result<()> {
    let source = match res {
        Resolution::Ours => ours,
        Resolution::Theirs => theirs,
        Resolution::Abort => bail!("aborted by user; conflict left in place"),
    };

    // If dest is read-only (e.g. LFS lockable), clear the bit before writing
    // and restore it after. This is spec §8 case 8a.
    let dest_meta = std::fs::metadata(dest).ok();
    let was_readonly = dest_meta
        .as_ref()
        .map(|m| m.permissions().readonly())
        .unwrap_or(false);
    if was_readonly {
        let mut perms = dest_meta.unwrap().permissions();
        perms.set_readonly(false);
        std::fs::set_permissions(dest, perms)?;
    }

    std::fs::copy(source, dest)?;

    if was_readonly {
        let mut perms = std::fs::metadata(dest)?.permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(dest, perms)?;
    }
    Ok(())
}
```

Update `app/src-tauri/src/lib.rs`:

```rust
pub mod diff;
pub mod git;
pub mod merge;
pub mod schema;
pub mod sidecar;
```

- [ ] **Step 4: Run merge tests**

```bash
cd app/src-tauri && cargo test --test merge_test
```

Expected: 4 PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri
git commit -m "feat(rust): apply_resolution with read-only LFS handling"
```

---

## Task 7: Merge driver installer/uninstaller

**Files:**
- Create: `app/src-tauri/src/installer.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Create: `app/src-tauri/tests/installer_test.rs`

Writes `.gitattributes` and `.git/config` entries idempotently. Removes them cleanly.

- [ ] **Step 1: Write the failing test**

Create `app/src-tauri/tests/installer_test.rs`:

```rust
use std::process::Command;
use tempfile::TempDir;
use unreal_merge::installer;

fn init_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(&path)
        .status()
        .unwrap();
    (dir, path)
}

#[test]
fn install_writes_gitattributes_and_config() {
    let (_tmp, repo) = init_repo();
    installer::install(&repo, &std::path::PathBuf::from("/usr/bin/unreal-merge")).unwrap();
    let attrs = std::fs::read_to_string(repo.join(".gitattributes")).unwrap();
    assert!(attrs.contains("*.uasset merge=unrealbin"));
    assert!(attrs.contains("*.umap   merge=unrealbin"));

    let cfg = std::fs::read_to_string(repo.join(".git").join("config")).unwrap();
    assert!(cfg.contains("[merge \"unrealbin\"]"));
    assert!(cfg.contains("--git-driver %O %A %B %P"));
}

#[test]
fn install_is_idempotent() {
    let (_tmp, repo) = init_repo();
    installer::install(&repo, &std::path::PathBuf::from("/usr/bin/unreal-merge")).unwrap();
    installer::install(&repo, &std::path::PathBuf::from("/usr/bin/unreal-merge")).unwrap();
    let attrs = std::fs::read_to_string(repo.join(".gitattributes")).unwrap();
    // Only one occurrence of our merge=unrealbin attribute (not two).
    assert_eq!(attrs.matches("*.uasset merge=unrealbin").count(), 1);
}

#[test]
fn uninstall_removes_attrs_and_config() {
    let (_tmp, repo) = init_repo();
    installer::install(&repo, &std::path::PathBuf::from("/usr/bin/unreal-merge")).unwrap();
    installer::uninstall(&repo).unwrap();
    let attrs = std::fs::read_to_string(repo.join(".gitattributes")).unwrap_or_default();
    assert!(!attrs.contains("merge=unrealbin"));
    let cfg = std::fs::read_to_string(repo.join(".git").join("config")).unwrap_or_default();
    assert!(!cfg.contains("[merge \"unrealbin\"]"));
}

#[test]
fn install_preserves_existing_gitattributes() {
    let (_tmp, repo) = init_repo();
    std::fs::write(repo.join(".gitattributes"), "*.txt text\n").unwrap();
    installer::install(&repo, &std::path::PathBuf::from("/usr/bin/unreal-merge")).unwrap();
    let attrs = std::fs::read_to_string(repo.join(".gitattributes")).unwrap();
    assert!(attrs.contains("*.txt text"));
    assert!(attrs.contains("*.uasset merge=unrealbin"));
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd app/src-tauri && cargo test --test installer_test
```

Expected: compile error — `installer` doesn't exist.

- [ ] **Step 3: Implement `installer.rs`**

Create `app/src-tauri/src/installer.rs`:

```rust
//! Install/uninstall the Git merge driver for *.uasset and *.umap conflicts.
//! Idempotent: running install twice yields the same file contents.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

const ATTR_MARK_BEGIN: &str = "# >>> unreal-merge driver (do not edit between markers) <<<";
const ATTR_MARK_END: &str = "# <<< unreal-merge driver >>>";
const ATTR_BODY: &str = "*.uasset merge=unrealbin\n*.umap   merge=unrealbin\n";

const CFG_SECTION: &str = "[merge \"unrealbin\"]";

pub fn install(repo: &Path, unreal_merge_exe: &Path) -> Result<()> {
    install_gitattributes(repo)?;
    install_git_config(repo, unreal_merge_exe)?;
    Ok(())
}

pub fn uninstall(repo: &Path) -> Result<()> {
    uninstall_gitattributes(repo)?;
    uninstall_git_config(repo)?;
    Ok(())
}

fn install_gitattributes(repo: &Path) -> Result<()> {
    let path = repo.join(".gitattributes");
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    if current.contains(ATTR_MARK_BEGIN) {
        return Ok(()); // already installed
    }
    let separator = if current.is_empty() || current.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let appended = format!(
        "{}{}{}\n{}{}\n",
        current, separator, ATTR_MARK_BEGIN, ATTR_BODY, ATTR_MARK_END
    );
    std::fs::write(&path, appended).context("write .gitattributes")?;
    Ok(())
}

fn uninstall_gitattributes(repo: &Path) -> Result<()> {
    let path = repo.join(".gitattributes");
    let current = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let mut out = String::new();
    let mut skipping = false;
    for line in current.lines() {
        if line.trim_end() == ATTR_MARK_BEGIN {
            skipping = true;
            continue;
        }
        if line.trim_end() == ATTR_MARK_END {
            skipping = false;
            continue;
        }
        if skipping {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    // Trim trailing blank lines for tidiness.
    while out.ends_with("\n\n") {
        out.pop();
    }
    std::fs::write(&path, out).context("rewrite .gitattributes")?;
    Ok(())
}

fn config_path(repo: &Path) -> PathBuf {
    repo.join(".git").join("config")
}

fn install_git_config(repo: &Path, exe: &Path) -> Result<()> {
    let path = config_path(repo);
    if !path.exists() {
        bail!("not a git repository: {} missing", path.display());
    }
    let current = std::fs::read_to_string(&path)?;
    if current.contains(CFG_SECTION) {
        return Ok(()); // already installed
    }
    let exe_display = exe.display().to_string().replace('\\', "/");
    let block = format!(
        "\n{}\n\tname = Unreal binary merge\n\tdriver = \"{}\" --git-driver %O %A %B %P\n\trecursive = binary\n",
        CFG_SECTION, exe_display
    );
    let mut updated = current;
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&block);
    std::fs::write(&path, updated)?;
    Ok(())
}

fn uninstall_git_config(repo: &Path) -> Result<()> {
    let path = config_path(repo);
    let current = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let mut out = String::new();
    let mut skipping = false;
    for line in current.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("[") {
            skipping = line.contains(CFG_SECTION);
            if skipping {
                continue;
            }
        }
        if skipping {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    std::fs::write(&path, out)?;
    Ok(())
}
```

Update `app/src-tauri/src/lib.rs`:

```rust
pub mod diff;
pub mod git;
pub mod installer;
pub mod merge;
pub mod schema;
pub mod sidecar;
```

- [ ] **Step 4: Run installer tests**

```bash
cd app/src-tauri && cargo test --test installer_test
```

Expected: 4 PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri
git commit -m "feat(rust): merge-driver installer with idempotent + reversible writes"
```

---

## Task 8: CLI surface

**Files:**
- Create: `app/src-tauri/src/cli.rs`
- Modify: `app/src-tauri/src/main.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Create: `app/src-tauri/tests/cli_test.rs`

Five subcommands: `install`, `uninstall`, `scan`, `export`, `--git-driver`. The git-driver mode is special (positional-args style, no subcommand) because Git invokes it that way.

- [ ] **Step 1: Write the failing CLI test**

Create `app/src-tauri/tests/cli_test.rs`:

```rust
use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn help_lists_subcommands() {
    Command::cargo_bin("unreal-merge")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("install"))
        .stdout(contains("uninstall"))
        .stdout(contains("scan"))
        .stdout(contains("export"));
}

#[test]
fn export_against_nonexistent_file_exits_nonzero() {
    // Without a sidecar reachable, we still want exit nonzero (not panic).
    let output = Command::cargo_bin("unreal-merge")
        .unwrap()
        .args([
            "export",
            "--sidecar",
            "C:/does/not/exist.exe",
            "C:/also/missing.uasset",
        ])
        .unwrap_err();
    let _ = output; // assert_cmd's `.unwrap_err` means the command exited nonzero — that's enough.
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd app/src-tauri && cargo test --test cli_test
```

Expected: build fails — the CLI doesn't accept those args yet.

- [ ] **Step 3: Implement `cli.rs`**

Create `app/src-tauri/src/cli.rs`:

```rust
//! Top-level CLI dispatch using clap.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::{diff::diff_properties, git, installer, merge, schema, sidecar};

#[derive(Parser, Debug)]
#[command(name = "unreal-merge", about = "Resolve UE binary merge conflicts")]
pub struct Cli {
    /// Git-driver mode: invoked positionally by Git's merge driver dispatch.
    /// When set, all four following positional arguments must be present.
    #[arg(long = "git-driver", num_args = 4, value_names = ["ANCESTOR", "OURS", "THEIRS", "PATH"])]
    pub git_driver: Option<Vec<String>>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Install the merge driver into the current Git repo.
    Install {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Remove the merge driver from the current Git repo.
    Uninstall {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// List conflicted .uasset/.umap files in the current repo.
    Scan {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Export one .uasset to JSON via the commandlet (debug helper).
    Export {
        /// Path to the .uasset to export.
        path: PathBuf,
        /// Override sidecar executable (defaults to UnrealEditor.exe lookup).
        #[arg(long)]
        sidecar: Option<PathBuf>,
        /// Override host project (defaults to ue-host/HostProject.uproject relative to cwd).
        #[arg(long)]
        host_project: Option<PathBuf>,
    },
    /// Compare two .uasset files via the commandlet and print property diffs.
    Diff {
        ours: PathBuf,
        theirs: PathBuf,
        #[arg(long)]
        sidecar: Option<PathBuf>,
        #[arg(long)]
        host_project: Option<PathBuf>,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    if let Some(args) = cli.git_driver {
        return run_git_driver(&args);
    }

    match cli.command {
        Some(Command::Install { repo }) => {
            let exe = std::env::current_exe().context("current_exe")?;
            installer::install(&repo, &exe)?;
            println!("Installed unreal-merge driver in {}", repo.display());
            Ok(())
        }
        Some(Command::Uninstall { repo }) => {
            installer::uninstall(&repo)?;
            println!("Uninstalled unreal-merge driver from {}", repo.display());
            Ok(())
        }
        Some(Command::Scan { repo }) => {
            let conflicts = git::list_conflicts(&repo)?;
            if conflicts.is_empty() {
                println!("No conflicts.");
            } else {
                for c in conflicts {
                    println!("{}", c);
                }
            }
            Ok(())
        }
        Some(Command::Export {
            path,
            sidecar,
            host_project,
        }) => run_export(&path, sidecar.as_deref(), host_project.as_deref()),
        Some(Command::Diff {
            ours,
            theirs,
            sidecar,
            host_project,
        }) => run_diff(&ours, &theirs, sidecar.as_deref(), host_project.as_deref()),
        None => {
            // No subcommand and no --git-driver: print help and exit 2.
            <Cli as clap::CommandFactory>::command().print_help()?;
            println!();
            std::process::exit(2);
        }
    }
}

fn default_sidecar() -> PathBuf {
    PathBuf::from(r"C:\Program Files\Epic Games\UE_5.6\Engine\Binaries\Win64\UnrealEditor.exe")
}

fn default_host_project() -> PathBuf {
    PathBuf::from("ue-host/HostProject.uproject")
}

fn build_sidecar(
    executable_override: Option<&std::path::Path>,
    host_project_override: Option<&std::path::Path>,
) -> sidecar::Sidecar {
    let executable = executable_override
        .map(PathBuf::from)
        .unwrap_or_else(default_sidecar);
    let host_project = host_project_override
        .map(PathBuf::from)
        .unwrap_or_else(default_host_project);
    // Mock sidecar takes no args; UE needs the project + commandlet flags.
    let args = if executable.to_string_lossy().to_lowercase().contains("unrealeditor") {
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
    let log_redirect = if executable
        .to_string_lossy()
        .to_lowercase()
        .contains("unrealeditor")
    {
        Some(std::env::temp_dir().join(format!(
            "unreal-merge-{}.log",
            std::process::id()
        )))
    } else {
        None
    };
    sidecar::Sidecar::new(sidecar::SidecarConfig {
        executable,
        args,
        prepend_warmup: true,
        log_redirect,
    })
}

fn export_via_sidecar(
    sidecar: &sidecar::Sidecar,
    path: &std::path::Path,
) -> Result<schema::AssetSnapshot> {
    let abs = std::fs::canonicalize(path).with_context(|| format!("canonicalise {}", path.display()))?;
    let path_str = abs.to_string_lossy().replace('\\', "/");
    let requests = vec![serde_json::json!({"id": 1, "cmd": "export", "path": path_str})];
    let responses = sidecar.run_batch(&requests)?;
    let response = responses
        .into_iter()
        .find(|r| r.get("id").and_then(|i| i.as_u64()) == Some(1))
        .context("no id=1 response from sidecar")?;
    let snap: schema::AssetSnapshot = serde_json::from_value(response)
        .context("parse AssetSnapshot")?;
    if !snap.ok {
        anyhow::bail!("commandlet reported ok=false");
    }
    Ok(snap)
}

fn run_export(
    path: &std::path::Path,
    sidecar_override: Option<&std::path::Path>,
    host_project_override: Option<&std::path::Path>,
) -> Result<()> {
    let s = build_sidecar(sidecar_override, host_project_override);
    let snap = export_via_sidecar(&s, path)?;
    println!("{}", serde_json::to_string_pretty(&snap)?);
    Ok(())
}

fn run_diff(
    ours: &std::path::Path,
    theirs: &std::path::Path,
    sidecar_override: Option<&std::path::Path>,
    host_project_override: Option<&std::path::Path>,
) -> Result<()> {
    let s = build_sidecar(sidecar_override, host_project_override);
    let snap_ours = export_via_sidecar(&s, ours)?;
    let snap_theirs = export_via_sidecar(&s, theirs)?;
    let diffs = diff_properties(&snap_ours.asset.properties, &snap_theirs.asset.properties);
    println!("ours saved_hash:   {}", snap_ours.package.saved_hash);
    println!("theirs saved_hash: {}", snap_theirs.package.saved_hash);
    if diffs.is_empty() {
        println!("No property-level diffs (hashes still differ - see Plan 1 done report).");
    } else {
        println!("Property diffs:");
        for d in diffs {
            println!("  {:?}", d);
        }
    }
    Ok(())
}

fn run_git_driver(args: &[String]) -> Result<()> {
    let [ancestor, ours, theirs, path] = match args {
        [a, b, c, d] => [a.clone(), b.clone(), c.clone(), d.clone()],
        _ => anyhow::bail!("--git-driver needs exactly 4 positional args"),
    };
    eprintln!("unreal-merge --git-driver:");
    eprintln!("  ancestor: {}", ancestor);
    eprintln!("  ours:     {}", ours);
    eprintln!("  theirs:   {}", theirs);
    eprintln!("  path:     {}", path);

    let resolution: merge::Resolution = std::env::var("UNREAL_MERGE_RESOLUTION")
        .unwrap_or_else(|_| "abort".to_string())
        .parse()?;
    eprintln!("  resolution from env: {:?}", resolution);

    let dest = std::path::PathBuf::from(&ours);
    match merge::apply_resolution(
        resolution,
        std::path::Path::new(&ours),
        std::path::Path::new(&theirs),
        &dest,
    ) {
        Ok(()) => {
            eprintln!("Resolution applied; exiting 0 (Git marks file resolved).");
            Ok(())
        }
        Err(e) => {
            eprintln!("Aborted ({}); exiting 1 (Git leaves conflict).", e);
            std::process::exit(1);
        }
    }
}
```

Update `app/src-tauri/src/lib.rs` and `app/src-tauri/src/main.rs`:

`lib.rs`:

```rust
//! Backend for unreal-merge.

pub mod cli;
pub mod diff;
pub mod git;
pub mod installer;
pub mod merge;
pub mod schema;
pub mod sidecar;
```

`main.rs`:

```rust
fn main() {
    if let Err(e) = unreal_merge::cli::run() {
        eprintln!("error: {:#}", e);
        std::process::exit(1);
    }
}
```

- [ ] **Step 4: Run all tests**

```bash
cd app/src-tauri && cargo test
```

Expected: all PASS (the new CLI tests plus everything that came before).

- [ ] **Step 5: Manually verify subcommands print sane help**

```bash
cd app/src-tauri
cargo run --bin unreal-merge -- --help
cargo run --bin unreal-merge -- install --help
cargo run --bin unreal-merge -- export --help
```

Each should print help text without panicking.

- [ ] **Step 6: Commit**

```bash
git add app/src-tauri
git commit -m "feat(rust): CLI surface (install/uninstall/scan/export/diff/--git-driver)"
```

---

## Task 9: End-to-end git-driver scenario against the mock sidecar

**Files:**
- Create: `app/src-tauri/tests/git_driver_e2e_test.rs`

This is the **acceptance test for Plan 2**: spin up a tmp git repo, trigger a `.uasset` conflict, run `unreal-merge --git-driver` with `UNREAL_MERGE_RESOLUTION=theirs`, confirm Git sees the file resolved.

- [ ] **Step 1: Write the test**

Create `app/src-tauri/tests/git_driver_e2e_test.rs`:

```rust
use assert_cmd::Command as AssertCommand;
use std::process::Command;
use tempfile::TempDir;

fn git(args: &[&str], cwd: &std::path::Path) {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .unwrap();
    assert!(status.success(), "git {:?} in {} failed", args, cwd.display());
}

#[test]
fn git_driver_theirs_resolves_uasset_conflict() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path();
    git(&["init", "-q"], repo);
    git(&["config", "user.email", "t@e.com"], repo);
    git(&["config", "user.name", "t"], repo);
    git(&["checkout", "-b", "main", "-q"], repo);
    std::fs::write(repo.join("a.uasset"), b"BASE").unwrap();
    git(&["add", "a.uasset"], repo);
    git(&["commit", "-q", "-m", "base"], repo);

    git(&["checkout", "-b", "feature", "-q"], repo);
    std::fs::write(repo.join("a.uasset"), b"FEATURE").unwrap();
    git(&["commit", "-q", "-am", "feature"], repo);

    git(&["checkout", "main", "-q"], repo);
    std::fs::write(repo.join("a.uasset"), b"MAIN").unwrap();
    git(&["commit", "-q", "-am", "main"], repo);

    // Attempt merge — expected to leave a conflict (binary file, no built-in merge).
    let _ = Command::new("git")
        .args(["merge", "feature", "--no-edit"])
        .current_dir(repo)
        .status();

    // Confirm the conflict actually landed.
    let ls = Command::new("git")
        .args(["ls-files", "-u"])
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&ls.stdout).contains("a.uasset"),
        "expected a.uasset in unmerged listing"
    );

    // Stage 2 = ours, stage 3 = theirs. Materialise them via git show.
    let ours_path = repo.join("ours.tmp");
    let theirs_path = repo.join("theirs.tmp");
    std::fs::write(
        &ours_path,
        Command::new("git")
            .args(["show", ":2:a.uasset"])
            .current_dir(repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    std::fs::write(
        &theirs_path,
        Command::new("git")
            .args(["show", ":3:a.uasset"])
            .current_dir(repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();

    // Invoke unreal-merge --git-driver with UNREAL_MERGE_RESOLUTION=theirs.
    AssertCommand::cargo_bin("unreal-merge")
        .unwrap()
        .env("UNREAL_MERGE_RESOLUTION", "theirs")
        .current_dir(repo)
        .args([
            "--git-driver",
            "<base-not-used-for-stage-copy>",
            ours_path.to_string_lossy().as_ref(),
            theirs_path.to_string_lossy().as_ref(),
            "a.uasset",
        ])
        .assert()
        .success();

    // The working tree's `ours.tmp` should now hold the contents of theirs ("FEATURE").
    // (Note: --git-driver's `%A` is the OURS path which Git also uses as the
    // destination after the driver returns. We mimic that here by writing the
    // resolved output back over ours_path.)
    let content = std::fs::read(&ours_path).unwrap();
    assert_eq!(content, b"FEATURE", "Resolution=Theirs should have written FEATURE");
}

#[test]
fn git_driver_abort_exits_nonzero() {
    let tmp = TempDir::new().unwrap();
    let ours = tmp.path().join("o.tmp");
    let theirs = tmp.path().join("t.tmp");
    std::fs::write(&ours, b"OURS").unwrap();
    std::fs::write(&theirs, b"THEIRS").unwrap();

    AssertCommand::cargo_bin("unreal-merge")
        .unwrap()
        .env("UNREAL_MERGE_RESOLUTION", "abort")
        .args([
            "--git-driver",
            "base-irrelevant",
            ours.to_string_lossy().as_ref(),
            theirs.to_string_lossy().as_ref(),
            "a.uasset",
        ])
        .assert()
        .failure(); // Git's signal that the conflict was NOT resolved.

    // Ours must NOT have been overwritten.
    assert_eq!(std::fs::read(&ours).unwrap(), b"OURS");
}
```

- [ ] **Step 2: Run the e2e tests**

```bash
cd app/src-tauri && cargo test --test git_driver_e2e_test
```

Expected: 2 PASS. Both run without UnrealEditor.exe because the git-driver path doesn't need the sidecar (it just file-copies the chosen side; the sidecar's role is for the UI's diff view in Plan 3).

- [ ] **Step 3: Commit**

```bash
git add app/src-tauri
git commit -m "test(rust): end-to-end --git-driver scenario with env-based resolution"
```

---

## Task 10: Smoke against real UE 5.6 + Plan 1 fixtures

**Files:**
- Create: `app/src-tauri/tests/real_ue_smoke.rs`

A single ignored test (`#[ignore]`) that, when run with `--ignored`, drives the actual UE 5.6 sidecar against `Examples/v1/BP_MinimalChar.uasset` and asserts we get back a snapshot with `class = "Blueprint"` and 40 properties. This is opt-in because UE invocations are slow and require the toolchain — but it provides the manual-acceptance gate for Plan 2 done criterion #2.

- [ ] **Step 1: Write the smoke test**

Create `app/src-tauri/tests/real_ue_smoke.rs`:

```rust
use std::path::PathBuf;
use unreal_merge::sidecar::{Sidecar, SidecarConfig};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Run with: cargo test --test real_ue_smoke -- --ignored --nocapture
#[test]
#[ignore]
fn export_v1_fixture_via_real_ue() {
    let root = repo_root();
    let ue = PathBuf::from(
        r"C:\Program Files\Epic Games\UE_5.6\Engine\Binaries\Win64\UnrealEditor.exe",
    );
    assert!(ue.exists(), "UE 5.6 not installed at {}", ue.display());

    let host_project = root.join("ue-host").join("HostProject.uproject");
    assert!(host_project.exists());

    let v1 = root.join("Examples").join("v1").join("BP_MinimalChar.uasset");
    assert!(v1.exists());

    let log_path = std::env::temp_dir().join("unreal-merge-smoke.log");
    let cfg = SidecarConfig {
        executable: ue,
        args: vec![
            host_project.to_string_lossy().to_string(),
            "-run=MergeBinariesExport".to_string(),
            "-stdio".to_string(),
            "-nullrhi".to_string(),
            "-unattended".to_string(),
            "-NoCrashReports".to_string(),
        ],
        prepend_warmup: true,
        log_redirect: Some(log_path),
    };
    let sidecar = Sidecar::new(cfg);

    let v1_str = v1.to_string_lossy().replace('\\', "/");
    let requests = vec![serde_json::json!({"id": 1, "cmd": "export", "path": v1_str})];
    let responses = sidecar.run_batch(&requests).expect("run batch");

    let response = responses
        .iter()
        .find(|r| r.get("id").and_then(|i| i.as_u64()) == Some(1))
        .expect("id=1 response from real UE 5.6 sidecar");

    assert_eq!(response["ok"], true, "got: {}", serde_json::to_string_pretty(response).unwrap());
    assert_eq!(response["asset"]["class"], "Blueprint");
    assert_eq!(response["package"]["fileVersionUE5"], 1017);
    let props = response["asset"]["properties"].as_array().expect("properties array");
    assert!(props.len() >= 30, "expected >= 30 properties, got {}", props.len());
}
```

- [ ] **Step 2: Run the smoke test against real UE**

```bash
cd app/src-tauri && cargo test --test real_ue_smoke -- --ignored --nocapture
```

Expected: 1 PASS. Takes ~30 seconds because UE boots from cold.

If this fails, the most likely cause is the `default_sidecar()` path mismatch — check that UE 5.6 is at `C:\Program Files\Epic Games\UE_5.6\` and that the `ue-host/Binaries/.../UnrealEditor-MergeBinariesExport.dll` from Plan 1 is present.

- [ ] **Step 3: Commit**

```bash
git add app/src-tauri
git commit -m "test(rust): ignored smoke against real UE 5.6 + Plan 1 fixtures"
```

---

## Done criteria — verify before declaring Plan 2 complete

Run from the repo root:

```bash
cd app/src-tauri && cargo test --all-targets
cd app/src-tauri && cargo test --test real_ue_smoke -- --ignored
cd app/src-tauri && cargo run --bin unreal-merge -- --help
cd app/src-tauri && cargo run --bin unreal-merge -- export "../../Examples/v1/BP_MinimalChar.uasset"
```

All four must succeed. The fourth prints a full property-walk JSON snapshot from the real UE 5.6 commandlet.

---

## Out of scope for Plan 2 (do NOT attempt)

- Tauri scaffolding / UI / IPC commands (Plan 3).
- 3-way diff (base/ours/theirs). 2-way (ours/theirs) is enough for the UI's first cut; 3-way lands when the design needs it.
- Long-lived sidecar (keep-warm across requests). Plan 2 spawns UE per batch.
- LFS lock auto-acquisition. Plan 2 only handles the read-only working file, with a warning.
- Per-property cherry-pick. Plan 4.
- Blueprint graph + componentTree + componentBindings (still under-the-Plan-4-hood; Plan 2 just round-trips whatever the schema says).
- Cross-platform support beyond Windows.
