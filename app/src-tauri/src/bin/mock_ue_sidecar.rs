//! Test double for the UE 5.6 MergeBinariesExport commandlet.
//!
//! Speaks the same JSON-RPC framing (newline-delimited JSON over stdio), supports
//! the same set of cmds (`ping`, `export`, `quit`), and emits a couple of fake log
//! lines on stdout before its first response — so consumers' brace-counter
//! extractors are exercised against realistic noise.
//!
//! The canned export returns a BP_Base Blueprint with one EventGraph that
//! mirrors the real BP_Base in ue-host/Content:
//!   BeginPlay → SET Health=0.0 → Branch (Condition fed via knot from Get Health)
//!               → True → PrintString (InString fed from same knot)
//! Ours (v1) adds a False-branch "Health was zero" PrintString.
//! Theirs (v2) feeds SET Health from a new MaxHealth getter.

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

const EVENT_GRAPH_OURS: &str = r#"Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name="K2Node_Event_BeginPlay"
   EventReference=(MemberParent=Class'"/Script/Engine.Actor"',MemberName="ReceiveBeginPlay")
   bOverrideFunction=True
   NodeGuid=00000000000000000000000000000001
   NodePosX=-80
   NodePosY=0
   CustomProperties Pin (PinId=A0000000000000000000000000000010,PinName="OutputDelegate",Direction="EGPD_Output",PinType.PinCategory="delegate",PinType.PinSubCategory="MulticastDelegateProperty",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
   CustomProperties Pin (PinId=A0000000000000000000000000000011,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health A0000000000000000000000000000020,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableSet Name="K2Node_VariableSet_Health"
   VariableReference=(MemberName="Health",MemberGuid=AABBCC00000000000000000000000001)
   NodeGuid=00000000000000000000000000000002
   NodePosX=180
   NodePosY=0
   CustomProperties Pin (PinId=A0000000000000000000000000000020,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Event_BeginPlay A0000000000000000000000000000011,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000021,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 A0000000000000000000000000000030,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000022,PinName="Health",Direction="EGPD_Input",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,DefaultValue="0.0",)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_IfThenElse Name="K2Node_IfThenElse_0"
   NodeGuid=00000000000000000000000000000003
   NodePosX=460
   NodePosY=0
   CustomProperties Pin (PinId=A0000000000000000000000000000030,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health A0000000000000000000000000000021,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000031,PinName="Condition",Direction="EGPD_Input",PinType.PinCategory="bool",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 A0000000000000000000000000000061,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000032,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_CallFunction_PrintTrue A0000000000000000000000000000040,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000033,PinName="else",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_CallFunction_PrintFalse A0000000000000000000000000000070,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name="K2Node_CallFunction_PrintTrue"
   FunctionReference=(MemberParent=Class'"/Script/Engine.KismetSystemLibrary"',MemberName="PrintString")
   NodeGuid=00000000000000000000000000000004
   NodePosX=760
   NodePosY=-100
   CustomProperties Pin (PinId=A0000000000000000000000000000040,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 A0000000000000000000000000000032,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000041,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
   CustomProperties Pin (PinId=A0000000000000000000000000000042,PinName="InString",Direction="EGPD_Input",PinType.PinCategory="string",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 A0000000000000000000000000000061,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableGet Name="K2Node_VariableGet_Health"
   VariableReference=(MemberName="Health",MemberGuid=AABBCC00000000000000000000000001)
   NodeGuid=00000000000000000000000000000005
   NodePosX=380
   NodePosY=220
   CustomProperties Pin (PinId=A0000000000000000000000000000050,PinName="Health",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 A0000000000000000000000000000060,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_Knot Name="K2Node_Knot_0"
   NodeGuid=00000000000000000000000000000006
   NodePosX=560
   NodePosY=180
   CustomProperties Pin (PinId=A0000000000000000000000000000060,PinName="InputPin",Direction="EGPD_Input",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableGet_Health A0000000000000000000000000000050,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000061,PinName="OutputPin",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 A0000000000000000000000000000031,K2Node_CallFunction_PrintTrue A0000000000000000000000000000042,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name="K2Node_CallFunction_PrintFalse"
   FunctionReference=(MemberParent=Class'"/Script/Engine.KismetSystemLibrary"',MemberName="PrintString")
   NodeGuid=00000000000000000000000000000007
   NodePosX=760
   NodePosY=100
   CustomProperties Pin (PinId=A0000000000000000000000000000070,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 A0000000000000000000000000000033,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000071,PinName="InString",Direction="EGPD_Input",PinType.PinCategory="string",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,DefaultValue="Health was zero",)
End Object
"#;

// Ancestor = common subset of OURS and THEIRS: BP_Base before either branch changed it.
// Has BeginPlay → SET Health=0.0 → Branch → True PrintString, with Knot from Get Health.
// NO False-branch PrintString (ours added), NO MaxHealth getter (theirs added).
// Pin IDs use the prefix "C0..." so neither side's pin prefix collides.
const EVENT_GRAPH_ANCESTOR: &str = r#"Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name="K2Node_Event_BeginPlay"
   EventReference=(MemberParent=Class'"/Script/Engine.Actor"',MemberName="ReceiveBeginPlay")
   bOverrideFunction=True
   NodeGuid=00000000000000000000000000000001
   NodePosX=-80
   NodePosY=0
   CustomProperties Pin (PinId=C0000000000000000000000000000010,PinName="OutputDelegate",Direction="EGPD_Output",PinType.PinCategory="delegate",PinType.PinSubCategory="MulticastDelegateProperty",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
   CustomProperties Pin (PinId=C0000000000000000000000000000011,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health C0000000000000000000000000000020,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableSet Name="K2Node_VariableSet_Health"
   VariableReference=(MemberName="Health",MemberGuid=AABBCC00000000000000000000000001)
   NodeGuid=00000000000000000000000000000002
   NodePosX=180
   NodePosY=0
   CustomProperties Pin (PinId=C0000000000000000000000000000020,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Event_BeginPlay C0000000000000000000000000000011,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000021,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 C0000000000000000000000000000030,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000022,PinName="Health",Direction="EGPD_Input",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,DefaultValue="0.0",)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_IfThenElse Name="K2Node_IfThenElse_0"
   NodeGuid=00000000000000000000000000000003
   NodePosX=460
   NodePosY=0
   CustomProperties Pin (PinId=C0000000000000000000000000000030,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health C0000000000000000000000000000021,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000031,PinName="Condition",Direction="EGPD_Input",PinType.PinCategory="bool",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 C0000000000000000000000000000061,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000032,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_CallFunction_PrintTrue C0000000000000000000000000000040,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000033,PinName="else",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name="K2Node_CallFunction_PrintTrue"
   FunctionReference=(MemberParent=Class'"/Script/Engine.KismetSystemLibrary"',MemberName="PrintString")
   NodeGuid=00000000000000000000000000000004
   NodePosX=760
   NodePosY=-100
   CustomProperties Pin (PinId=C0000000000000000000000000000040,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 C0000000000000000000000000000032,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000041,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
   CustomProperties Pin (PinId=C0000000000000000000000000000042,PinName="InString",Direction="EGPD_Input",PinType.PinCategory="string",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 C0000000000000000000000000000061,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableGet Name="K2Node_VariableGet_Health"
   VariableReference=(MemberName="Health",MemberGuid=AABBCC00000000000000000000000001)
   NodeGuid=00000000000000000000000000000005
   NodePosX=380
   NodePosY=220
   CustomProperties Pin (PinId=C0000000000000000000000000000050,PinName="Health",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 C0000000000000000000000000000060,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_Knot Name="K2Node_Knot_0"
   NodeGuid=00000000000000000000000000000006
   NodePosX=560
   NodePosY=180
   CustomProperties Pin (PinId=C0000000000000000000000000000060,PinName="InputPin",Direction="EGPD_Input",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableGet_Health C0000000000000000000000000000050,),)
   CustomProperties Pin (PinId=C0000000000000000000000000000061,PinName="OutputPin",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 C0000000000000000000000000000031,K2Node_CallFunction_PrintTrue C0000000000000000000000000000042,),)
End Object
"#;

const EVENT_GRAPH_THEIRS: &str = r#"Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name="K2Node_Event_BeginPlay"
   EventReference=(MemberParent=Class'"/Script/Engine.Actor"',MemberName="ReceiveBeginPlay")
   bOverrideFunction=True
   NodeGuid=00000000000000000000000000000001
   NodePosX=-80
   NodePosY=0
   CustomProperties Pin (PinId=B0000000000000000000000000000010,PinName="OutputDelegate",Direction="EGPD_Output",PinType.PinCategory="delegate",PinType.PinSubCategory="MulticastDelegateProperty",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
   CustomProperties Pin (PinId=B0000000000000000000000000000011,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health B0000000000000000000000000000020,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableSet Name="K2Node_VariableSet_Health"
   VariableReference=(MemberName="Health",MemberGuid=AABBCC00000000000000000000000001)
   NodeGuid=00000000000000000000000000000002
   NodePosX=180
   NodePosY=0
   CustomProperties Pin (PinId=B0000000000000000000000000000020,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Event_BeginPlay B0000000000000000000000000000011,),)
   CustomProperties Pin (PinId=B0000000000000000000000000000021,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 B0000000000000000000000000000030,),)
   CustomProperties Pin (PinId=B0000000000000000000000000000022,PinName="Health",Direction="EGPD_Input",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,DefaultValue="0.0",LinkedTo=(K2Node_VariableGet_MaxHealth B0000000000000000000000000000080,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_IfThenElse Name="K2Node_IfThenElse_0"
   NodeGuid=00000000000000000000000000000003
   NodePosX=460
   NodePosY=0
   CustomProperties Pin (PinId=B0000000000000000000000000000030,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health B0000000000000000000000000000021,),)
   CustomProperties Pin (PinId=B0000000000000000000000000000031,PinName="Condition",Direction="EGPD_Input",PinType.PinCategory="bool",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 B0000000000000000000000000000061,),)
   CustomProperties Pin (PinId=B0000000000000000000000000000032,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_CallFunction_PrintTrue B0000000000000000000000000000040,),)
   CustomProperties Pin (PinId=B0000000000000000000000000000033,PinName="else",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name="K2Node_CallFunction_PrintTrue"
   FunctionReference=(MemberParent=Class'"/Script/Engine.KismetSystemLibrary"',MemberName="PrintString")
   NodeGuid=00000000000000000000000000000004
   NodePosX=760
   NodePosY=-100
   CustomProperties Pin (PinId=B0000000000000000000000000000040,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 B0000000000000000000000000000032,),)
   CustomProperties Pin (PinId=B0000000000000000000000000000041,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
   CustomProperties Pin (PinId=B0000000000000000000000000000042,PinName="InString",Direction="EGPD_Input",PinType.PinCategory="string",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 B0000000000000000000000000000061,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableGet Name="K2Node_VariableGet_Health"
   VariableReference=(MemberName="Health",MemberGuid=AABBCC00000000000000000000000001)
   NodeGuid=00000000000000000000000000000005
   NodePosX=380
   NodePosY=220
   CustomProperties Pin (PinId=B0000000000000000000000000000050,PinName="Health",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 B0000000000000000000000000000060,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_Knot Name="K2Node_Knot_0"
   NodeGuid=00000000000000000000000000000006
   NodePosX=560
   NodePosY=180
   CustomProperties Pin (PinId=B0000000000000000000000000000060,PinName="InputPin",Direction="EGPD_Input",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableGet_Health B0000000000000000000000000000050,),)
   CustomProperties Pin (PinId=B0000000000000000000000000000061,PinName="OutputPin",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 B0000000000000000000000000000031,K2Node_CallFunction_PrintTrue B0000000000000000000000000000042,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableGet Name="K2Node_VariableGet_MaxHealth"
   VariableReference=(MemberName="MaxHealth",MemberGuid=CCDDEE00000000000000000000000001)
   NodeGuid=00000000000000000000000000000008
   NodePosX=-20
   NodePosY=140
   CustomProperties Pin (PinId=B0000000000000000000000000000080,PinName="MaxHealth",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health B0000000000000000000000000000022,),)
End Object
"#;

fn handle_export(path: &str, id: Option<&serde_json::Value>) -> serde_json::Value {
    let lower = path.to_lowercase();
    let is_theirs = lower.contains("v2") || lower.contains("theirs");
    let is_ancestor = lower.contains("ancestor") || lower.contains("base") || lower.contains("\\o\\") || lower.contains("/o/");

    let (event_graph, hash, default_health) = if is_ancestor {
        (EVENT_GRAPH_ANCESTOR, "sha1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 50.0)
    } else if is_theirs {
        (EVENT_GRAPH_THEIRS, "sha1:1111111111111111111111111111111111111111", 100.0)
    } else {
        (EVENT_GRAPH_OURS, "sha1:0000000000000000000000000000000000000000", 0.0)
    };

    let mut resp = serde_json::json!({
        "ok": true,
        "path": path,
        "package": {
            "name": "/Game/BP_Base",
            "engineVersion": "5.6.0-mock+++UE5+Release-5.6",
            "fileVersionUE5": 1017,
            "savedHash": hash
        },
        "asset": {
            "class": "Blueprint",
            "parentClass": "/Script/Engine.Actor",
            "name": "BP_Base",
            "properties": [
                {"path": "DefaultHealth", "type": "float", "value": default_health},
                {"path": "MaxHealth", "type": "float", "value": 100.0}
            ],
            "graphs": {
                "EventGraph": event_graph
            }
        }
    });
    if let Some(id_val) = id {
        resp["id"] = id_val.clone();
    }
    resp
}

fn handle_merge(req: &serde_json::Value, id: Option<&serde_json::Value>) -> serde_json::Value {
    let merged_graphs = req
        .get("mergedGraphs")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    // Write the concatenation of all merged graph texts to a temp file.
    // The mock doesn't produce a real .uasset — the consumer just copies
    // the bytes over `dest`, which is fine for IPC exercise.
    let mut merged_text = String::new();
    for (name, value) in &merged_graphs {
        merged_text.push_str(&format!("// graph: {}\n", name));
        if let Some(s) = value.as_str() {
            merged_text.push_str(s);
        }
        merged_text.push('\n');
    }

    let temp_dir = std::env::temp_dir();
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let merged_path = temp_dir.join(format!("unreal-merge-mock-{}-{}.uasset", pid, nanos));

    if let Err(e) = std::fs::write(&merged_path, merged_text.as_bytes()) {
        let mut resp = serde_json::json!({"ok": false, "error": format!("write temp: {}", e)});
        if let Some(v) = id { resp["id"] = v.clone(); }
        return resp;
    }

    let mut resp = serde_json::json!({
        "ok": true,
        "mergedPath": merged_path.to_string_lossy().replace('\\', "/"),
    });
    if let Some(v) = id { resp["id"] = v.clone(); }
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
            Some("_warmup") => continue,
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
            Some("merge") => {
                write_json(&handle_merge(&req, id.as_ref()));
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
