#pragma once

#include "CoreMinimal.h"
#include "Dom/JsonObject.h"

class FAssetExporter
{
public:
    /**
     * Export the asset at `AbsoluteAssetPath` (a path to a .uasset file on disk).
     * Populates `OutResponse` with `{ok:true, asset:{...}}` or `{ok:false, error:"..."}`.
     */
    static void Export(const FString& AbsoluteAssetPath, TSharedRef<FJsonObject>& OutResponse);

private:
    /** Build the `package` block. Returns nullptr on failure (and populates `OutError`). */
    static TSharedPtr<FJsonObject> BuildPackageBlock(const FString& AbsoluteAssetPath,
                                                     UPackage* Package,
                                                     FString& OutError);
};
