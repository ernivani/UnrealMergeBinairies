#pragma once

#include "CoreMinimal.h"
#include "Dom/JsonObject.h"

class FMergeApplier
{
public:
    // Reads request fields:
    //   path: string (ancestor .uasset path on disk)
    //   mergedGraphs: object { graphName: string of UE serialization text }
    //
    // Duplicates the ancestor asset to a temp path, imports merged nodes into
    // each named graph (replacing existing nodes), recompiles, saves package,
    // and writes:
    //   { ok: true, mergedPath: string } on success
    //   { ok: false, error: string } on failure
    static void Apply(const TSharedPtr<FJsonObject>& Req, TSharedRef<FJsonObject>& OutResponse);
};
