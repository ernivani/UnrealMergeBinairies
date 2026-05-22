#include "AssetExporter.h"

#include "HAL/FileManager.h"
#include "Misc/EngineVersion.h"
#include "Misc/FileHelper.h"
#include "Misc/PackageName.h"
#include "Misc/Paths.h"
#include "Misc/SecureHash.h"
#include "UObject/Package.h"
#include "UObject/UObjectGlobals.h"

namespace
{
    // Display-name reported in the JSON `package.name` field. When we have a real
    // /Game/... long package name (asset lives inside an Unreal project's Content/),
    // we use that. When we synthesise a temporary mount root (asset is outside any
    // project — our test fixtures), we report the stable form /MergeTmp/<basename>
    // so the JSON is byte-identical across runs regardless of which numeric mount
    // counter UE happens to be at internally.
    FString StableDisplayName(const FString& AbsoluteAssetPath, bool bUsedTempMount, const FString& RealPackageName)
    {
        if (!bUsedTempMount)
        {
            return RealPackageName;
        }
        return TEXT("/MergeTmp/") + FPaths::GetBaseFilename(AbsoluteAssetPath);
    }
}

void FAssetExporter::Export(const FString& AbsoluteAssetPath, TSharedRef<FJsonObject>& OutResponse)
{
    if (!FPaths::FileExists(AbsoluteAssetPath))
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"),
            FString::Printf(TEXT("file not found: %s"), *AbsoluteAssetPath));
        return;
    }

    // Each Export call gets its OWN mount root. Re-registering /MergeTmp/ at a
    // different directory does not invalidate UE's package cache, so without this
    // the second export of a same-named asset returns the first call's UPackage
    // (and in Task 5 will look like the property walk produced identical output
    // for v1 and v2). The numeric counter guarantees we hit LoadPackage fresh.
    static int32 MountCounter = 0;
    FString PackageName;
    FString MountRoot;        // empty if we used a real long package name
    FString MountTargetDir;

    if (!FPackageName::TryConvertFilenameToLongPackageName(AbsoluteAssetPath, PackageName))
    {
        const FString BaseName = FPaths::GetBaseFilename(AbsoluteAssetPath);
        const FString DirPath  = FPaths::GetPath(AbsoluteAssetPath);
        MountRoot       = FString::Printf(TEXT("/MergeTmp%d/"), ++MountCounter);
        MountTargetDir  = DirPath + TEXT("/");
        FPackageName::RegisterMountPoint(MountRoot, MountTargetDir);
        PackageName = MountRoot + BaseName;
    }

    UPackage* Package = LoadPackage(nullptr, *PackageName, LOAD_NoWarn | LOAD_Quiet);
    if (!Package)
    {
        if (!MountRoot.IsEmpty()) { FPackageName::UnRegisterMountPoint(MountRoot, MountTargetDir); }
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"),
            FString::Printf(TEXT("LoadPackage failed for %s"), *PackageName));
        return;
    }

    const FString DisplayName = StableDisplayName(AbsoluteAssetPath, !MountRoot.IsEmpty(), Package->GetName());

    FString Error;
    const TSharedPtr<FJsonObject> PackageBlock = BuildPackageBlock(AbsoluteAssetPath, DisplayName, Error);
    if (!PackageBlock.IsValid())
    {
        if (!MountRoot.IsEmpty()) { FPackageName::UnRegisterMountPoint(MountRoot, MountTargetDir); }
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

    // Unmount the temporary root. The loaded UPackage stays valid until GC, and
    // each call's package lives under a distinct mount root so cleanup never
    // affects an earlier call's result. We deliberately do NOT call CollectGarbage
    // here — repeated CollectGarbage in a warm commandlet is expensive and the
    // mount-counter strategy already prevents cache collisions.
    if (!MountRoot.IsEmpty()) { FPackageName::UnRegisterMountPoint(MountRoot, MountTargetDir); }
}

TSharedPtr<FJsonObject> FAssetExporter::BuildPackageBlock(const FString& AbsoluteAssetPath,
                                                          const FString& DisplayName,
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
    Out->SetStringField(TEXT("name"),            DisplayName);
    Out->SetStringField(TEXT("engineVersion"),   FEngineVersion::Current().ToString());
    Out->SetNumberField(TEXT("fileVersionUE5"),  static_cast<double>(FileVersionUE5));
    Out->SetStringField(TEXT("savedHash"),       FString::Printf(TEXT("sha1:%s"), *Hex));
    return Out;
}
