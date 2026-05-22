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
