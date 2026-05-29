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
     * - either the package's real long name, or a stable synthesised form when we
     * loaded the asset through a temporary mount root (keeps goldens run-independent).
     * Returns nullptr on failure (and populates `OutError`).
     */
    static TSharedPtr<FJsonObject> BuildPackageBlock(const FString& AbsoluteAssetPath,
                                                     const FString& DisplayName,
                                                     FString& OutError);

    /** Recursive walk; appends one entry per leaf-valued FProperty. */
    static void WalkProperties(const void* ContainerData,
                               UStruct* Struct,
                               const FString& PathPrefix,
                               TArray<TSharedPtr<FJsonValue>>& OutEntries,
                               int32 Depth = 0);

    /** Serialise one property's value to a JsonValue. Returns nullptr for unsupported. */
    static TSharedPtr<FJsonValue> SerialisePropertyValue(FProperty* Property, const void* ValuePtr);

    /** Find the asset's primary UObject inside a UPackage (the "main" asset; what UE shows in the Content Browser). */
    static UObject* FindPrimaryAsset(UPackage* Package);

    static constexpr int32 MaxRecursionDepth = 8;
};
