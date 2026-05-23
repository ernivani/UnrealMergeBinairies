/**
 * Dev-only smoke test for the ueblueprint renderer.
 * Visible at http://localhost:1420 when running `pnpm dev` in the browser.
 *
 * Mocks a BP_Base merge conflict, mirroring the real graph layout:
 *   BeginPlay → SET Health=0.0 → Branch (Condition fed from Get Health via knot)
 *               → True → PrintString (InString fed from same knot)
 *               → False → (Ours: PrintString "Health was zero" — Theirs: unconnected)
 *
 *   Ours (Alice):  added a False-branch PrintString.
 *   Theirs (Bob):  fed SET Health from a new Get MaxHealth node.
 *
 * Pin IDs must be exactly 32 hex chars (UE GuidEntity); node GUIDs likewise.
 */
import type { AssetSnapshot, GraphDiff } from "../types";
import GraphView from "./GraphView";

// Stable across both sides (used to compute diff status).
const G_BEGINPLAY   = "00000000000000000000000000000001";
const G_SET_HEALTH  = "00000000000000000000000000000002";
const G_BRANCH      = "00000000000000000000000000000003";
const G_PRINT_TRUE  = "00000000000000000000000000000004";
const G_GET_HEALTH  = "00000000000000000000000000000005";
const G_KNOT        = "00000000000000000000000000000006";
const G_PRINT_FALSE = "00000000000000000000000000000007"; // ours-only
const G_GET_MAX     = "00000000000000000000000000000008"; // theirs-only

const EVENT_GRAPH_OURS = `\
Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name="K2Node_Event_BeginPlay"
   EventReference=(MemberParent=Class'"/Script/Engine.Actor"',MemberName="ReceiveBeginPlay")
   bOverrideFunction=True
   NodeGuid=${G_BEGINPLAY}
   NodePosX=-80
   NodePosY=0
   CustomProperties Pin (PinId=A0000000000000000000000000000010,PinName="OutputDelegate",Direction="EGPD_Output",PinType.PinCategory="delegate",PinType.PinSubCategory="MulticastDelegateProperty",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
   CustomProperties Pin (PinId=A0000000000000000000000000000011,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health A0000000000000000000000000000020,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableSet Name="K2Node_VariableSet_Health"
   VariableReference=(MemberName="Health",MemberGuid=AABBCC00000000000000000000000001)
   NodeGuid=${G_SET_HEALTH}
   NodePosX=180
   NodePosY=0
   CustomProperties Pin (PinId=A0000000000000000000000000000020,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Event_BeginPlay A0000000000000000000000000000011,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000021,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 A0000000000000000000000000000030,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000022,PinName="Health",Direction="EGPD_Input",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,DefaultValue="0.0",)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_IfThenElse Name="K2Node_IfThenElse_0"
   NodeGuid=${G_BRANCH}
   NodePosX=460
   NodePosY=0
   CustomProperties Pin (PinId=A0000000000000000000000000000030,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health A0000000000000000000000000000021,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000031,PinName="Condition",Direction="EGPD_Input",PinType.PinCategory="bool",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 A0000000000000000000000000000061,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000032,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_CallFunction_PrintTrue A0000000000000000000000000000040,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000033,PinName="else",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_CallFunction_PrintFalse A0000000000000000000000000000070,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name="K2Node_CallFunction_PrintTrue"
   FunctionReference=(MemberParent=Class'"/Script/Engine.KismetSystemLibrary"',MemberName="PrintString")
   NodeGuid=${G_PRINT_TRUE}
   NodePosX=760
   NodePosY=-100
   CustomProperties Pin (PinId=A0000000000000000000000000000040,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 A0000000000000000000000000000032,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000041,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
   CustomProperties Pin (PinId=A0000000000000000000000000000042,PinName="InString",Direction="EGPD_Input",PinType.PinCategory="string",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 A0000000000000000000000000000061,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableGet Name="K2Node_VariableGet_Health"
   VariableReference=(MemberName="Health",MemberGuid=AABBCC00000000000000000000000001)
   NodeGuid=${G_GET_HEALTH}
   NodePosX=380
   NodePosY=220
   CustomProperties Pin (PinId=A0000000000000000000000000000050,PinName="Health",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 A0000000000000000000000000000060,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_Knot Name="K2Node_Knot_0"
   NodeGuid=${G_KNOT}
   NodePosX=560
   NodePosY=180
   CustomProperties Pin (PinId=A0000000000000000000000000000060,PinName="InputPin",Direction="EGPD_Input",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableGet_Health A0000000000000000000000000000050,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000061,PinName="OutputPin",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 A0000000000000000000000000000031,K2Node_CallFunction_PrintTrue A0000000000000000000000000000042,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name="K2Node_CallFunction_PrintFalse"
   FunctionReference=(MemberParent=Class'"/Script/Engine.KismetSystemLibrary"',MemberName="PrintString")
   NodeGuid=${G_PRINT_FALSE}
   NodePosX=760
   NodePosY=100
   CustomProperties Pin (PinId=A0000000000000000000000000000070,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 A0000000000000000000000000000033,),)
   CustomProperties Pin (PinId=A0000000000000000000000000000071,PinName="InString",Direction="EGPD_Input",PinType.PinCategory="string",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,DefaultValue="Health was zero",)
End Object
`;

const EVENT_GRAPH_THEIRS = `\
Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name="K2Node_Event_BeginPlay"
   EventReference=(MemberParent=Class'"/Script/Engine.Actor"',MemberName="ReceiveBeginPlay")
   bOverrideFunction=True
   NodeGuid=${G_BEGINPLAY}
   NodePosX=-80
   NodePosY=0
   CustomProperties Pin (PinId=B0000000000000000000000000000010,PinName="OutputDelegate",Direction="EGPD_Output",PinType.PinCategory="delegate",PinType.PinSubCategory="MulticastDelegateProperty",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
   CustomProperties Pin (PinId=B0000000000000000000000000000011,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health B0000000000000000000000000000020,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableSet Name="K2Node_VariableSet_Health"
   VariableReference=(MemberName="Health",MemberGuid=AABBCC00000000000000000000000001)
   NodeGuid=${G_SET_HEALTH}
   NodePosX=180
   NodePosY=0
   CustomProperties Pin (PinId=B0000000000000000000000000000020,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Event_BeginPlay B0000000000000000000000000000011,),)
   CustomProperties Pin (PinId=B0000000000000000000000000000021,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 B0000000000000000000000000000030,),)
   CustomProperties Pin (PinId=B0000000000000000000000000000022,PinName="Health",Direction="EGPD_Input",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,DefaultValue="0.0",LinkedTo=(K2Node_VariableGet_MaxHealth B0000000000000000000000000000080,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_IfThenElse Name="K2Node_IfThenElse_0"
   NodeGuid=${G_BRANCH}
   NodePosX=460
   NodePosY=0
   CustomProperties Pin (PinId=B0000000000000000000000000000030,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health B0000000000000000000000000000021,),)
   CustomProperties Pin (PinId=B0000000000000000000000000000031,PinName="Condition",Direction="EGPD_Input",PinType.PinCategory="bool",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 B0000000000000000000000000000061,),)
   CustomProperties Pin (PinId=B0000000000000000000000000000032,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_CallFunction_PrintTrue B0000000000000000000000000000040,),)
   CustomProperties Pin (PinId=B0000000000000000000000000000033,PinName="else",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name="K2Node_CallFunction_PrintTrue"
   FunctionReference=(MemberParent=Class'"/Script/Engine.KismetSystemLibrary"',MemberName="PrintString")
   NodeGuid=${G_PRINT_TRUE}
   NodePosX=760
   NodePosY=-100
   CustomProperties Pin (PinId=B0000000000000000000000000000040,PinName="execute",Direction="EGPD_Input",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 B0000000000000000000000000000032,),)
   CustomProperties Pin (PinId=B0000000000000000000000000000041,PinName="then",Direction="EGPD_Output",PinType.PinCategory="exec",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,)
   CustomProperties Pin (PinId=B0000000000000000000000000000042,PinName="InString",Direction="EGPD_Input",PinType.PinCategory="string",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 B0000000000000000000000000000061,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableGet Name="K2Node_VariableGet_Health"
   VariableReference=(MemberName="Health",MemberGuid=AABBCC00000000000000000000000001)
   NodeGuid=${G_GET_HEALTH}
   NodePosX=380
   NodePosY=220
   CustomProperties Pin (PinId=B0000000000000000000000000000050,PinName="Health",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_Knot_0 B0000000000000000000000000000060,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_Knot Name="K2Node_Knot_0"
   NodeGuid=${G_KNOT}
   NodePosX=560
   NodePosY=180
   CustomProperties Pin (PinId=B0000000000000000000000000000060,PinName="InputPin",Direction="EGPD_Input",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableGet_Health B0000000000000000000000000000050,),)
   CustomProperties Pin (PinId=B0000000000000000000000000000061,PinName="OutputPin",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_IfThenElse_0 B0000000000000000000000000000031,K2Node_CallFunction_PrintTrue B0000000000000000000000000000042,),)
End Object
Begin Object Class=/Script/BlueprintGraph.K2Node_VariableGet Name="K2Node_VariableGet_MaxHealth"
   VariableReference=(MemberName="MaxHealth",MemberGuid=CCDDEE00000000000000000000000001)
   NodeGuid=${G_GET_MAX}
   NodePosX=-20
   NodePosY=140
   CustomProperties Pin (PinId=B0000000000000000000000000000080,PinName="MaxHealth",Direction="EGPD_Output",PinType.PinCategory="float",PinType.bIsConst=False,PinType.bIsReference=False,PinType.bIsWeakPointer=False,LinkedTo=(K2Node_VariableSet_Health B0000000000000000000000000000022,),)
End Object
`;

function makeSnapshot(graphs: Record<string, string>): AssetSnapshot {
  return {
    ok: true,
    path: "/mock",
    package: {
      name: "/Game/BP_Base",
      engineVersion: "5.6.0-mock",
      fileVersionUE5: 1017,
      savedHash: "sha1:0000000000000000000000000000000000000000",
    },
    asset: {
      class: "Blueprint",
      parentClass: "/Script/Engine.Actor",
      name: "BP_Base",
      properties: [],
      graphs,
    },
  };
}

const OURS = makeSnapshot({ EventGraph: EVENT_GRAPH_OURS });
const THEIRS = makeSnapshot({ EventGraph: EVENT_GRAPH_THEIRS });

const DIFFS: GraphDiff[] = [
  {
    name: "EventGraph",
    onlyInOurs: false,
    onlyInTheirs: false,
    nodeStatuses: {
      [G_BEGINPLAY]:   "unchanged",
      [G_SET_HEALTH]:  "changed",
      [G_BRANCH]:      "unchanged",
      [G_PRINT_TRUE]:  "unchanged",
      [G_GET_HEALTH]:  "unchanged",
      [G_KNOT]:        "unchanged",
      [G_PRINT_FALSE]: "removed",
      [G_GET_MAX]:     "added",
    },
  },
];

export default function BlueprintTest() {
  return (
    <div
      style={{
        height: "100vh",
        display: "flex",
        flexDirection: "column",
        background: "var(--ue-bg-deep)",
      }}
    >
      <div
        style={{
          padding: "8px 14px",
          background: "linear-gradient(to bottom, #1f1f1f, #161616)",
          borderBottom: "1px solid var(--ue-border)",
          fontSize: 11,
          color: "var(--ue-text-dim)",
          letterSpacing: "0.04em",
        }}
      >
        BP_Base merge conflict — Alice (Ours) adds a False-branch PrintString; Bob (Theirs) feeds SET Health from a new MaxHealth getter.
      </div>
      <GraphView ours={OURS} theirs={THEIRS} graphDiffs={DIFFS} />
    </div>
  );
}
