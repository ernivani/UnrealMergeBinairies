#include "AssetExporter.h"

#include "HAL/FileManager.h"
#include "Misc/EngineVersion.h"
#include "Misc/FileHelper.h"
#include "Misc/PackageName.h"
#include "Misc/Paths.h"
#include "Misc/SecureHash.h"
#include "UObject/Package.h"
#include "UObject/UObjectGlobals.h"
#include "UObject/UnrealType.h"
#include "UObject/TextProperty.h"
#include "UObject/EnumProperty.h"
#include "UObject/SoftObjectPtr.h"
#include "Dom/JsonValue.h"
#include "Misc/PackagePath.h"

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

    // Replace counter-suffixed temporary mount roots (/MergeTmp17/...) with the stable
    // form (/MergeTmp/...) so subobject references in serialised property values stay
    // run-independent across the lifetime of the commandlet.
    FString NormaliseObjectPath(const FString& Path)
    {
        if (Path.IsEmpty()) { return Path; }
        // Find a "/MergeTmp<digits>/" prefix or anywhere-in-string occurrence and rewrite.
        FString Result = Path;
        int32 Idx = Result.Find(TEXT("/MergeTmp"));
        while (Idx != INDEX_NONE)
        {
            const int32 DigitStart = Idx + 9; // length of "/MergeTmp"
            int32 DigitEnd = DigitStart;
            while (DigitEnd < Result.Len() && FChar::IsDigit(Result[DigitEnd])) { ++DigitEnd; }
            if (DigitEnd > DigitStart && DigitEnd < Result.Len() && Result[DigitEnd] == TEXT('/'))
            {
                Result = Result.Left(DigitStart) + Result.Mid(DigitEnd);
            }
            Idx = Result.Find(TEXT("/MergeTmp"), ESearchCase::CaseSensitive, ESearchDir::FromStart, Idx + 9);
        }
        return Result;
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

    // Construct an FPackagePath that carries the explicit .uasset header extension.
    // Without an explicit extension, the UE 5.5 linker silently refuses to open
    // packages reached via a runtime-registered mount root.
    FPackagePath PkgPath = FPackagePath::FromPackageNameChecked(*PackageName);
    PkgPath.SetHeaderExtension(EPackageExtension::Asset);

    UPackage* Package = LoadPackage(nullptr, PkgPath, LOAD_None);
    if (!Package)
    {
        if (!MountRoot.IsEmpty()) { FPackageName::UnRegisterMountPoint(MountRoot, MountTargetDir); }
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"),
            FString::Printf(TEXT("LoadPackage failed for %s"), *PackageName));
        return;
    }
    // Ensure the package is fully loaded — under -nullrhi commandlets LoadPackage can
    // return a partially-populated UPackage with no inner objects until FullyLoad runs.
    Package->FullyLoad();

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

    UObject* Primary = FindPrimaryAsset(Package);
    if (!Primary)
    {
        if (!MountRoot.IsEmpty()) { FPackageName::UnRegisterMountPoint(MountRoot, MountTargetDir); }
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"),
            FString::Printf(TEXT("no primary asset found in package %s"), *PackageName));
        return;
    }

    const TSharedRef<FJsonObject> Asset = MakeShared<FJsonObject>();
    Asset->SetStringField(TEXT("class"), Primary->GetClass()->GetName());
    Asset->SetStringField(TEXT("parentClass"),
        Primary->GetClass()->GetSuperClass()
            ? Primary->GetClass()->GetSuperClass()->GetPathName()
            : FString());
    Asset->SetStringField(TEXT("name"), Primary->GetName());

    TArray<TSharedPtr<FJsonValue>> Entries;
    WalkProperties(Primary, Primary->GetClass(), FString(), Entries);

    // Sort by `path` so the JSON is canonical (equal inputs -> byte-identical output).
    Entries.Sort([](const TSharedPtr<FJsonValue>& A, const TSharedPtr<FJsonValue>& B) {
        return A->AsObject()->GetStringField(TEXT("path"))
             < B->AsObject()->GetStringField(TEXT("path"));
    });

    Asset->SetArrayField(TEXT("properties"), Entries);

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

UObject* FAssetExporter::FindPrimaryAsset(UPackage* Package)
{
    // Prefer the object whose name matches the package shortname AND is RF_Standalone —
    // that is what UE itself treats as the package's "primary asset".
    UObject* Found = nullptr;
    const FString ShortName = FPackageName::GetShortName(Package->GetName());

    ForEachObjectWithOuter(Package, [&Found, &ShortName](UObject* It)
    {
        if (Found) { return; }
        // UE renamed UMetaData → UPackageMetaData around 5.6; match by class name to stay version-independent.
        const FName ClassName = It->GetClass()->GetFName();
        if (ClassName == TEXT("MetaData") || ClassName == TEXT("PackageMetaData")) { return; }
        if (It->HasAnyFlags(RF_Standalone) && It->GetName() == ShortName)
        {
            Found = It;
        }
    }, /*bIncludeNestedObjects=*/false);
    if (Found) { return Found; }

    // Fallbacks for assets without a standard name/flag layout.
    ForEachObjectWithOuter(Package, [&Found](UObject* It)
    {
        if (Found) { return; }
        // UE renamed UMetaData → UPackageMetaData around 5.6; match by class name to stay version-independent.
        const FName ClassName = It->GetClass()->GetFName();
        if (ClassName == TEXT("MetaData") || ClassName == TEXT("PackageMetaData")) { return; }
        if (It->HasAnyFlags(RF_Standalone | RF_Public)) { Found = It; }
    }, /*bIncludeNestedObjects=*/false);
    return Found;
}

TSharedPtr<FJsonValue> FAssetExporter::SerialisePropertyValue(FProperty* Property, const void* ValuePtr)
{
    if (FBoolProperty*  P = CastField<FBoolProperty>(Property))  { return MakeShared<FJsonValueBoolean>(P->GetPropertyValue(ValuePtr)); }
    if (FIntProperty*   P = CastField<FIntProperty>(Property))   { return MakeShared<FJsonValueNumber>(static_cast<double>(P->GetPropertyValue(ValuePtr))); }
    if (FInt64Property* P = CastField<FInt64Property>(Property)) { return MakeShared<FJsonValueNumber>(static_cast<double>(P->GetPropertyValue(ValuePtr))); }
    if (FFloatProperty* P = CastField<FFloatProperty>(Property)) { return MakeShared<FJsonValueNumber>(static_cast<double>(P->GetPropertyValue(ValuePtr))); }
    if (FDoubleProperty* P = CastField<FDoubleProperty>(Property)) { return MakeShared<FJsonValueNumber>(P->GetPropertyValue(ValuePtr)); }
    if (FStrProperty*   P = CastField<FStrProperty>(Property))   { return MakeShared<FJsonValueString>(P->GetPropertyValue(ValuePtr)); }
    if (FNameProperty*  P = CastField<FNameProperty>(Property))  { return MakeShared<FJsonValueString>(P->GetPropertyValue(ValuePtr).ToString()); }
    if (FTextProperty*  P = CastField<FTextProperty>(Property))  { return MakeShared<FJsonValueString>(P->GetPropertyValue(ValuePtr).ToString()); }
    if (FEnumProperty*  P = CastField<FEnumProperty>(Property))
    {
        const int64 Value = P->GetUnderlyingProperty()->GetSignedIntPropertyValue(ValuePtr);
        const UEnum* EnumDef = P->GetEnum();
        const FString Name = EnumDef ? EnumDef->GetNameStringByValue(Value) : FString::FromInt(Value);
        return MakeShared<FJsonValueString>(Name);
    }
    if (FObjectProperty* P = CastField<FObjectProperty>(Property))
    {
        UObject* Obj = P->GetObjectPropertyValue(ValuePtr);
        return MakeShared<FJsonValueString>(NormaliseObjectPath(Obj ? Obj->GetPathName() : FString()));
    }
    if (FSoftObjectProperty* P = CastField<FSoftObjectProperty>(Property))
    {
        const FSoftObjectPtr& Ptr = *static_cast<const FSoftObjectPtr*>(ValuePtr);
        return MakeShared<FJsonValueString>(NormaliseObjectPath(Ptr.ToString()));
    }
    if (FStructProperty* P = CastField<FStructProperty>(Property))
    {
        const TSharedRef<FJsonObject> Summary = MakeShared<FJsonObject>();
        Summary->SetStringField(TEXT("type"),    TEXT("struct"));
        Summary->SetStringField(TEXT("summary"), P->Struct->GetName());
        return MakeShared<FJsonValueObject>(Summary);
    }
    if (FArrayProperty* P = CastField<FArrayProperty>(Property))
    {
        FScriptArrayHelper Helper(P, ValuePtr);
        const TSharedRef<FJsonObject> Summary = MakeShared<FJsonObject>();
        Summary->SetStringField(TEXT("type"),    TEXT("array"));
        Summary->SetNumberField(TEXT("length"),  Helper.Num());
        Summary->SetStringField(TEXT("element"), P->Inner->GetCPPType());
        return MakeShared<FJsonValueObject>(Summary);
    }
    if (FMapProperty* P = CastField<FMapProperty>(Property))
    {
        FScriptMapHelper Helper(P, ValuePtr);
        const TSharedRef<FJsonObject> Summary = MakeShared<FJsonObject>();
        Summary->SetStringField(TEXT("type"),   TEXT("map"));
        Summary->SetNumberField(TEXT("length"), Helper.Num());
        return MakeShared<FJsonValueObject>(Summary);
    }
    if (FSetProperty* P = CastField<FSetProperty>(Property))
    {
        FScriptSetHelper Helper(P, ValuePtr);
        const TSharedRef<FJsonObject> Summary = MakeShared<FJsonObject>();
        Summary->SetStringField(TEXT("type"),   TEXT("set"));
        Summary->SetNumberField(TEXT("length"), Helper.Num());
        return MakeShared<FJsonValueObject>(Summary);
    }
    return nullptr;
}

void FAssetExporter::WalkProperties(const void* ContainerData,
                                    UStruct* Struct,
                                    const FString& PathPrefix,
                                    TArray<TSharedPtr<FJsonValue>>& OutEntries,
                                    int32 Depth)
{
    if (Depth > MaxRecursionDepth || !Struct || !ContainerData)
    {
        return;
    }

    for (TFieldIterator<FProperty> It(Struct); It; ++It)
    {
        FProperty* Property = *It;
        if (!Property || Property->HasAnyPropertyFlags(CPF_Transient | CPF_Deprecated))
        {
            continue;
        }

        const FString PropertyPath = PathPrefix.IsEmpty()
            ? Property->GetName()
            : PathPrefix + TEXT(".") + Property->GetName();

        const void* ValuePtr = Property->ContainerPtrToValuePtr<void>(ContainerData);
        TSharedPtr<FJsonValue> Value = SerialisePropertyValue(Property, ValuePtr);

        if (Value.IsValid())
        {
            const TSharedRef<FJsonObject> Entry = MakeShared<FJsonObject>();
            Entry->SetStringField(TEXT("path"),  PropertyPath);
            Entry->SetStringField(TEXT("type"),  Property->GetCPPType());
            Entry->SetField   (TEXT("value"), Value);
            OutEntries.Add(MakeShared<FJsonValueObject>(Entry));
        }
        // Note: we intentionally do NOT recurse into FStructProperty here in Plan 1.
        // Spec keeps property paths flat with structs as opaque summaries; the
        // detailed struct-field walk is Plan 4's responsibility.
    }
}
