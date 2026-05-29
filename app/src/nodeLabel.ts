// Derive a human-readable label for a Blueprint node from its UE serialization
// blob (the text between `Begin Object` / `End Object`). Pure + tested.

function member(blob: string, re: RegExp): string | undefined {
  const m = blob.match(re);
  return m ? m[1] : undefined;
}

export function nodeLabel(blob: string): string {
  if (!blob) return "node";

  const cls = member(blob, /Begin Object Class=\S*?\.(\w+)/) ?? "";

  switch (cls) {
    case "K2Node_IfThenElse":
      return "Branch";
    case "K2Node_Knot":
      return "Reroute";
    case "K2Node_VariableSet": {
      const v = member(blob, /VariableReference=\(MemberName="([^"]+)"/);
      return v ? `SET ${v}` : "Set Variable";
    }
    case "K2Node_VariableGet": {
      const v = member(blob, /VariableReference=\(MemberName="([^"]+)"/);
      return v ? `GET ${v}` : "Get Variable";
    }
    case "K2Node_Event":
    case "K2Node_CustomEvent": {
      const e =
        member(blob, /EventReference=\([^)]*?MemberName="([^"]+)"/) ??
        member(blob, /CustomFunctionName="([^"]+)"/);
      return e ? `Event ${e.replace(/^Receive/, "")}` : "Event";
    }
    case "K2Node_CallFunction":
    case "K2Node_CallArrayFunction":
    case "K2Node_CallParentFunction": {
      const f = member(blob, /FunctionReference=\([^)]*?MemberName="([^"]+)"/);
      return f ?? "Call Function";
    }
    case "K2Node_DynamicCast": {
      const t = member(blob, /TargetType=[^."]*\.(\w+)"/);
      return t ? `Cast ${t}` : "Cast";
    }
    default:
      return cls ? cls.replace(/^K2Node_/, "") : "node";
  }
}
