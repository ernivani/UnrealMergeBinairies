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

// ── EventGraph ────────────────────────────────────────────────────────────────
// Shared nodes (both sides): BeginPlay event, Branch, PrintString, SetHealth
// Changed (same GUID, different wiring/values): Branch condition, PrintString message
// Removed (only in Ours): a legacy "DebugLog" call
// Added (only in Theirs): new "ApplyDamage" call + a Sequence node

const EVENT_GRAPH_OURS: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name=\"K2Node_Event_0\"\n\
   EventReference=(MemberParent=Class'\"/Script/Engine.Actor\"',MemberName=\"ReceiveBeginPlay\")\n\
   NodeGuid=00000001000000000000000000000001\n\
   NodePosX=0\n\
   NodePosY=0\n\
   NodeComment=\"Entry point\"\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_IfThenElse Name=\"K2Node_IfThenElse_0\"\n\
   NodeGuid=00000002000000000000000000000002\n\
   NodePosX=250\n\
   NodePosY=0\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_0\"\n\
   FunctionReference=(MemberParent=Class'\"/Script/Engine.KismetSystemLibrary\"',MemberName=\"PrintString\")\n\
   NodeGuid=00000003000000000000000000000003\n\
   NodePosX=500\n\
   NodePosY=-80\n\
   Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_0\"\n\
      InString=\"Hello from Ours!\"\n\
      Duration=2.0\n\
   End Object\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_1\"\n\
   FunctionReference=(MemberParent=Class'\"/Script/Engine.KismetSystemLibrary\"',MemberName=\"PrintString\")\n\
   NodeGuid=00000004000000000000000000000004\n\
   NodePosX=500\n\
   NodePosY=80\n\
   Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_1\"\n\
      InString=\"DEBUG: legacy log\"\n\
      Duration=5.0\n\
   End Object\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_2\"\n\
   FunctionReference=(MemberName=\"SetActorHiddenInGame\")\n\
   NodeGuid=00000005000000000000000000000005\n\
   NodePosX=750\n\
   NodePosY=0\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableSet Name=\"K2Node_VariableSet_0\"\n\
   VariableReference=(MemberName=\"Health\",MemberGuid=AABB00000000000000000000000000AA)\n\
   NodeGuid=00000006000000000000000000000006\n\
   NodePosX=1000\n\
   NodePosY=0\n\
End Object\n\
";

const EVENT_GRAPH_THEIRS: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name=\"K2Node_Event_0\"\n\
   EventReference=(MemberParent=Class'\"/Script/Engine.Actor\"',MemberName=\"ReceiveBeginPlay\")\n\
   NodeGuid=00000001000000000000000000000001\n\
   NodePosX=0\n\
   NodePosY=0\n\
   NodeComment=\"Entry point\"\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_IfThenElse Name=\"K2Node_IfThenElse_0\"\n\
   NodeGuid=00000002000000000000000000000002\n\
   NodePosX=250\n\
   NodePosY=0\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_0\"\n\
   FunctionReference=(MemberParent=Class'\"/Script/Engine.KismetSystemLibrary\"',MemberName=\"PrintString\")\n\
   NodeGuid=00000003000000000000000000000003\n\
   NodePosX=500\n\
   NodePosY=-80\n\
   Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_0\"\n\
      InString=\"Hello from Theirs — updated message!\"\n\
      Duration=4.0\n\
   End Object\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_ExecutionSequence Name=\"K2Node_ExecutionSequence_0\"\n\
   NodeGuid=00000007000000000000000000000007\n\
   NodePosX=500\n\
   NodePosY=80\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_3\"\n\
   FunctionReference=(MemberName=\"ApplyDamage\")\n\
   NodeGuid=00000008000000000000000000000008\n\
   NodePosX=750\n\
   NodePosY=80\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_2\"\n\
   FunctionReference=(MemberName=\"SetActorHiddenInGame\")\n\
   NodeGuid=00000005000000000000000000000005\n\
   NodePosX=750\n\
   NodePosY=0\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableSet Name=\"K2Node_VariableSet_0\"\n\
   VariableReference=(MemberName=\"Health\",MemberGuid=AABB00000000000000000000000000AA)\n\
   NodeGuid=00000006000000000000000000000006\n\
   NodePosX=1000\n\
   NodePosY=0\n\
End Object\n\
";

// ── TakeDamage function graph ─────────────────────────────────────────────────
// Ours: simple clamp + set. Theirs: added a death-check branch after the clamp.

const TAKE_DAMAGE_OURS: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_FunctionEntry Name=\"K2Node_FunctionEntry_0\"\n\
   NodeGuid=00000010000000000000000000000010\n\
   NodePosX=0\n\
   NodePosY=0\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_10\"\n\
   FunctionReference=(MemberParent=Class'\"/Script/Engine.KismetMathLibrary\"',MemberName=\"FClamp\")\n\
   NodeGuid=00000011000000000000000000000011\n\
   NodePosX=250\n\
   NodePosY=0\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableSet Name=\"K2Node_VariableSet_1\"\n\
   VariableReference=(MemberName=\"Health\")\n\
   NodeGuid=00000012000000000000000000000012\n\
   NodePosX=500\n\
   NodePosY=0\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_FunctionResult Name=\"K2Node_FunctionResult_0\"\n\
   NodeGuid=00000013000000000000000000000013\n\
   NodePosX=750\n\
   NodePosY=0\n\
End Object\n\
";

const TAKE_DAMAGE_THEIRS: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_FunctionEntry Name=\"K2Node_FunctionEntry_0\"\n\
   NodeGuid=00000010000000000000000000000010\n\
   NodePosX=0\n\
   NodePosY=0\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_10\"\n\
   FunctionReference=(MemberParent=Class'\"/Script/Engine.KismetMathLibrary\"',MemberName=\"FClamp\")\n\
   NodeGuid=00000011000000000000000000000011\n\
   NodePosX=250\n\
   NodePosY=0\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableSet Name=\"K2Node_VariableSet_1\"\n\
   VariableReference=(MemberName=\"Health\")\n\
   NodeGuid=00000012000000000000000000000012\n\
   NodePosX=500\n\
   NodePosY=0\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_IfThenElse Name=\"K2Node_IfThenElse_1\"\n\
   NodeGuid=00000014000000000000000000000014\n\
   NodePosX=750\n\
   NodePosY=0\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_11\"\n\
   FunctionReference=(MemberName=\"Die\")\n\
   NodeGuid=00000015000000000000000000000015\n\
   NodePosX=1000\n\
   NodePosY=-60\n\
End Object\n\
Begin Object Class=/Script/BlueprintGraph.K2Node_FunctionResult Name=\"K2Node_FunctionResult_0\"\n\
   NodeGuid=00000013000000000000000000000013\n\
   NodePosX=1000\n\
   NodePosY=60\n\
End Object\n\
";

fn handle_export(path: &str, id: Option<&serde_json::Value>) -> serde_json::Value {
    let is_theirs = path.contains("v2");
    let (event_graph, take_damage, hash, bool_val, str_val) = if is_theirs {
        (EVENT_GRAPH_THEIRS, TAKE_DAMAGE_THEIRS,
         "sha1:1111111111111111111111111111111111111111",
         true, "theirs-value")
    } else {
        (EVENT_GRAPH_OURS, TAKE_DAMAGE_OURS,
         "sha1:0000000000000000000000000000000000000000",
         false, "ours-value")
    };

    let mut resp = serde_json::json!({
        "ok": true,
        "path": path,
        "package": {
            "name": "/Game/BP_Character",
            "engineVersion": "5.6.0-mock+++UE5+Release-5.6",
            "fileVersionUE5": 1017,
            "savedHash": hash
        },
        "asset": {
            "class": "Blueprint",
            "parentClass": "/Script/Engine.Character",
            "name": "BP_Character",
            "properties": [
                {"path": "MaxHealth", "type": "float", "value": 100.0},
                {"path": "bInvincible", "type": "bool", "value": bool_val},
                {"path": "CharacterName", "type": "FString", "value": str_val},
                {"path": "MoveSpeed", "type": "float", "value": if is_theirs { 450.0 } else { 400.0 }}
            ],
            "graphs": {
                "EventGraph": event_graph,
                "TakeDamage": take_damage
            }
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
            Some("_warmup") => {
                // Silently absorb the warmup frame so the mock mirrors UE's
                // first-stdin-line eating behavior.
                continue;
            }
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
