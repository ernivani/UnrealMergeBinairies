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
    /**
     * Build the `package` block. `DisplayName` is the value to emit for `package.name`
     * — either the package's real long name, or a stable synthesised form when we
     * loaded the asset through a temporary mount root (keeps goldens run-independent).
     * Returns nullptr on failure (and populates `OutError`).
     */
    static TSharedPtr<FJsonObject> BuildPackageBlock(const FString& AbsoluteAssetPath,
                                                     const FString& DisplayName,
                                                     FString& OutError);
};
