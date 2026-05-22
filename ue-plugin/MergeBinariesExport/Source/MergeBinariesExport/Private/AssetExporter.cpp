#include "AssetExporter.h"

#include "HAL/FileManager.h"
#include "Misc/EngineVersion.h"
#include "Misc/FileHelper.h"
#include "Misc/PackageName.h"
#include "Misc/Paths.h"
#include "Misc/SecureHash.h"
#include "UObject/Package.h"
#include "UObject/UObjectGlobals.h"

void FAssetExporter::Export(const FString& AbsoluteAssetPath, TSharedRef<FJsonObject>& OutResponse)
{
    if (!FPaths::FileExists(AbsoluteAssetPath))
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"),
            FString::Printf(TEXT("file not found: %s"), *AbsoluteAssetPath));
        return;
    }

    // Map the on-disk path into a UE package mount. UE's loader uses /Game/... style paths.
    FString PackageName;
    if (!FPackageName::TryConvertFilenameToLongPackageName(AbsoluteAssetPath, PackageName))
    {
        // Fall back: mount a synthetic prefix at the asset's directory so LoadPackage works.
        const FString BaseName  = FPaths::GetBaseFilename(AbsoluteAssetPath);
        const FString DirPath   = FPaths::GetPath(AbsoluteAssetPath);
        const FString MountRoot = TEXT("/MergeTmp/");
        FPackageName::RegisterMountPoint(MountRoot, DirPath + TEXT("/"));
        PackageName = MountRoot + BaseName;
    }

    UPackage* Package = LoadPackage(nullptr, *PackageName, LOAD_NoWarn | LOAD_Quiet);
    if (!Package)
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"),
            FString::Printf(TEXT("LoadPackage failed for %s"), *PackageName));
        return;
    }

    FString Error;
    const TSharedPtr<FJsonObject> PackageBlock = BuildPackageBlock(AbsoluteAssetPath, Package, Error);
    if (!PackageBlock.IsValid())
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"), Error);
        return;
    }

    const TSharedRef<FJsonObject> Asset = MakeShared<FJsonObject>();
    // `asset` block is filled by Task 5; for now leave a placeholder so consumers can detect the schema version.
    Asset->SetStringField(TEXT("class"), TEXT("(unknown - pending Task 5)"));

    OutResponse->SetBoolField(TEXT("ok"), true);
    OutResponse->SetStringField(TEXT("path"), AbsoluteAssetPath);
    OutResponse->SetObjectField(TEXT("package"), PackageBlock);
    OutResponse->SetObjectField(TEXT("asset"), Asset);
}

TSharedPtr<FJsonObject> FAssetExporter::BuildPackageBlock(const FString& AbsoluteAssetPath,
                                                          UPackage* Package,
                                                          FString& OutError)
{
    TArray<uint8> Bytes;
    if (!FFileHelper::LoadFileToArray(Bytes, *AbsoluteAssetPath))
    {
        OutError = TEXT("could not read .uasset bytes for hashing");
        return nullptr;
    }

    FSHA1 Sha;
    Sha.Update(Bytes.GetData(), Bytes.Num());
    Sha.Final();
    uint8 Digest[FSHA1::DigestSize];
    Sha.GetHash(Digest);
    FString Hex;
    for (int32 i = 0; i < FSHA1::DigestSize; ++i)
    {
        Hex += FString::Printf(TEXT("%02x"), Digest[i]);
    }

    // Read FileVersionUE5 directly from the on-disk FPackageFileSummary.
    // Layout: int32 Tag | int32 LegacyFileVersion | int32 LegacyUE3Version | int32 FileVersionUE4 | int32 FileVersionUE5
    // (FileVersionUE5 is at byte offset 16; this matches the v5.5 saved-summary format with LegacyFileVersion == -8.)
    // The linker's Summary is not reliably reachable after LoadPackage completes (linkers may be detached),
    // so reading the file ourselves is the deterministic source of truth.
    int32 FileVersionUE5 = 0;
    if (Bytes.Num() >= 20)
    {
        FMemory::Memcpy(&FileVersionUE5, Bytes.GetData() + 16, sizeof(int32));
    }

    const TSharedRef<FJsonObject> Out = MakeShared<FJsonObject>();
    Out->SetStringField(TEXT("name"),            Package->GetName());
    Out->SetStringField(TEXT("engineVersion"),   FEngineVersion::Current().ToString());
    Out->SetNumberField(TEXT("fileVersionUE5"),  static_cast<double>(FileVersionUE5));
    Out->SetStringField(TEXT("savedHash"),       FString::Printf(TEXT("sha1:%s"), *Hex));
    return Out;
}
