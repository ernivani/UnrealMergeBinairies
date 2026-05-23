#include "MergeApplier.h"

#include "Engine/Blueprint.h"
#include "EdGraph/EdGraph.h"
#include "EdGraph/EdGraphNode.h"
#include "EdGraphUtilities.h"
#include "HAL/FileManager.h"
#include "Kismet2/BlueprintEditorUtils.h"
#include "Kismet2/KismetEditorUtilities.h"
#include "Misc/FileHelper.h"
#include "Misc/Guid.h"
#include "Misc/Paths.h"
#include "UObject/Package.h"
#include "UObject/SavePackage.h"

namespace
{
    bool DuplicatePackageToTemp(const FString& SrcDiskPath, FString& OutTempDiskPath, UPackage*& OutPackage, FString& OutError)
    {
        if (!FPaths::FileExists(SrcDiskPath))
        {
            OutError = FString::Printf(TEXT("ancestor not found: %s"), *SrcDiskPath);
            return false;
        }

        // Load the source package.
        const FString PackageName = FPackageName::FilenameToLongPackageName(SrcDiskPath);
        UPackage* SrcPackage = LoadPackage(nullptr, *PackageName, LOAD_None);
        if (!SrcPackage)
        {
            OutError = FString::Printf(TEXT("LoadPackage failed for %s"), *PackageName);
            return false;
        }

        // Build a unique temp package name + disk path.
        const FString IntermediateDir = FPaths::ProjectIntermediateDir() / TEXT("UnrealMerge");
        IFileManager::Get().MakeDirectory(*IntermediateDir, /*Tree=*/true);

        const FString UniqueId = FGuid::NewGuid().ToString(EGuidFormats::Short);
        const FString TempPackageName = FString::Printf(TEXT("/Temp/UnrealMerge/Merged_%s"), *UniqueId);
        OutTempDiskPath = IntermediateDir / FString::Printf(TEXT("Merged_%s.uasset"), *UniqueId);

        // Duplicate the package.
        OutPackage = CreatePackage(*TempPackageName);
        if (!OutPackage)
        {
            OutError = TEXT("CreatePackage for temp failed");
            return false;
        }

        // Duplicate every UObject in the source package into the dest package.
        for (TObjectIterator<UObject> It; It; ++It)
        {
            UObject* Obj = *It;
            if (Obj && Obj->GetOutermost() == SrcPackage && !Obj->IsTemplate(RF_ClassDefaultObject))
            {
                StaticDuplicateObject(Obj, OutPackage, Obj->GetFName());
            }
        }

        return true;
    }

    UBlueprint* FindBlueprintInPackage(UPackage* Package)
    {
        for (TObjectIterator<UBlueprint> It; It; ++It)
        {
            if (It->GetOutermost() == Package)
            {
                return *It;
            }
        }
        return nullptr;
    }

    UEdGraph* FindGraphByName(UBlueprint* BP, const FString& Name)
    {
        for (UEdGraph* G : BP->UbergraphPages)   { if (G && G->GetName() == Name) return G; }
        for (UEdGraph* G : BP->FunctionGraphs)   { if (G && G->GetName() == Name) return G; }
        for (UEdGraph* G : BP->MacroGraphs)      { if (G && G->GetName() == Name) return G; }
        return nullptr;
    }

    bool ReplaceGraphNodes(UEdGraph* Graph, const FString& MergedText, FString& OutError)
    {
        // Remove all existing nodes.
        Graph->Modify();
        for (int32 i = Graph->Nodes.Num() - 1; i >= 0; --i)
        {
            UEdGraphNode* N = Graph->Nodes[i];
            if (N)
            {
                Graph->RemoveNode(N);
            }
        }

        // Import merged nodes.
        TSet<UEdGraphNode*> Imported;
        FEdGraphUtilities::ImportNodesFromText(Graph, MergedText, /*out*/ Imported);
        if (Imported.Num() == 0 && !MergedText.IsEmpty())
        {
            OutError = TEXT("ImportNodesFromText produced 0 nodes");
            return false;
        }
        Graph->NotifyGraphChanged();
        return true;
    }
}

void FMergeApplier::Apply(const TSharedPtr<FJsonObject>& Req, TSharedRef<FJsonObject>& OutResponse)
{
    FString AncestorPath;
    if (!Req->TryGetStringField(TEXT("path"), AncestorPath))
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"), TEXT("missing 'path'"));
        return;
    }

    const TSharedPtr<FJsonObject>* MergedGraphsObj = nullptr;
    if (!Req->TryGetObjectField(TEXT("mergedGraphs"), MergedGraphsObj) || !MergedGraphsObj)
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"), TEXT("missing 'mergedGraphs'"));
        return;
    }

    FString TempDiskPath;
    UPackage* TempPackage = nullptr;
    FString Err;
    if (!DuplicatePackageToTemp(AncestorPath, TempDiskPath, TempPackage, Err))
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"), Err);
        return;
    }

    UBlueprint* BP = FindBlueprintInPackage(TempPackage);
    if (!BP)
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"), TEXT("no Blueprint in duplicated package"));
        return;
    }

    for (const auto& Kv : (*MergedGraphsObj)->Values)
    {
        const FString& GraphName = Kv.Key;
        FString MergedText;
        if (Kv.Value.IsValid() && Kv.Value->TryGetString(MergedText))
        {
            if (UEdGraph* G = FindGraphByName(BP, GraphName))
            {
                if (!ReplaceGraphNodes(G, MergedText, Err))
                {
                    OutResponse->SetBoolField(TEXT("ok"), false);
                    OutResponse->SetStringField(TEXT("error"),
                        FString::Printf(TEXT("graph '%s': %s"), *GraphName, *Err));
                    return;
                }
            }
            // Graphs in the request but not on the asset are silently skipped.
        }
    }

    // Best-effort recompile — log on failure but continue.
    FKismetEditorUtilities::CompileBlueprint(BP, EBlueprintCompileOptions::SkipGarbageCollection);

    // Save package.
    FSavePackageArgs SaveArgs;
    SaveArgs.TopLevelFlags = RF_Public | RF_Standalone;
    SaveArgs.SaveFlags = SAVE_NoError;
    SaveArgs.Error = GError;
    const bool bSaved = UPackage::SavePackage(TempPackage, BP, *TempDiskPath, SaveArgs);
    if (!bSaved)
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"), TEXT("SavePackage failed"));
        return;
    }

    OutResponse->SetBoolField(TEXT("ok"), true);
    OutResponse->SetStringField(TEXT("mergedPath"), TempDiskPath.Replace(TEXT("\\"), TEXT("/")));
}
