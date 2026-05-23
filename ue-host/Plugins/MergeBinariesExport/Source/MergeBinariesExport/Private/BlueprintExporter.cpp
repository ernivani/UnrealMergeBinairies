#include "BlueprintExporter.h"

#include "Engine/Blueprint.h"
#include "EdGraph/EdGraph.h"
#include "Kismet2/BlueprintEditorUtils.h"

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

		TSet<UEdGraphNode*> NodeSet(Graph->Nodes);
		FString ExportedText;
		FBlueprintEditorUtils::ExportNodesToText(NodeSet, ExportedText);

		FGraphExport Export;
		Export.GraphName = Graph->GetName();
		Export.GraphText = MoveTemp(ExportedText);
		Result.Add(MoveTemp(Export));
	}
	return Result;
}
