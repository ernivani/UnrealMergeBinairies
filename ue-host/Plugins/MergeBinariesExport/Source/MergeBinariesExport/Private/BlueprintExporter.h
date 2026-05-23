#pragma once

#include "CoreMinimal.h"
#include "Dom/JsonObject.h"

class UBlueprint;

struct FGraphExport
{
	FString GraphName;
	FString GraphText;
};

class FBlueprintExporter
{
public:
	// Returns one FGraphExport per graph (EventGraph, function graphs, macro graphs).
	// Returns empty array if Blueprint is null or has no graphs.
	static TArray<FGraphExport> ExportGraphs(UBlueprint* Blueprint);
};
