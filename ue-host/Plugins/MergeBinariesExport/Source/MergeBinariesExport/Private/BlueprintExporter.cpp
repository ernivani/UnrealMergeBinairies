#include "BlueprintExporter.h"

#include "Engine/Blueprint.h"
#include "EdGraph/EdGraph.h"
#include "EdGraph/EdGraphNode.h"
#include "EdGraphUtilities.h"

TArray<FGraphExport> FBlueprintExporter::ExportGraphs(UBlueprint* Blueprint)
{
	TArray<FGraphExport> Result;
	if (!Blueprint) { return Result; }

	TArray<UEdGraph*> AllGraphs;
	AllGraphs.Append(Blueprint->UbergraphPages);
	AllGraphs.Append(Blueprint->FunctionGraphs);
	AllGraphs.Append(Blueprint->MacroGraphs);

	for (UEdGraph* Graph : AllGraphs)
	{
		if (!Graph) { continue; }

		// FEdGraphUtilities::ExportNodesToText takes a TSet<UObject*>.
		TSet<UObject*> NodeSet;
		NodeSet.Reserve(Graph->Nodes.Num());
		for (UEdGraphNode* Node : Graph->Nodes)
		{
			if (Node) { NodeSet.Add(Node); }
		}

		FString ExportedText;
		FEdGraphUtilities::ExportNodesToText(NodeSet, ExportedText);

		FGraphExport Export;
		Export.GraphName = Graph->GetName();
		Export.GraphText = MoveTemp(ExportedText);
		Result.Add(MoveTemp(Export));
	}
	return Result;
}
