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
#include "Misc/PackageName.h"
#include "Misc/Paths.h"
#include "UObject/Package.h"
#include "UObject/SavePackage.h"

namespace
{
    UBlueprint* FindBlueprintInPackage(UPackage* Package)
    {
        UBlueprint* Found = nullptr;
        ForEachObjectWithOuter(Package, [&Found](UObject* It)
        {
            if (!Found)
            {
                if (UBlueprint* BP = Cast<UBlueprint>(It)) { Found = BP; }
            }
        }, /*bIncludeNestedObjects=*/false);
        return Found;
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
        Graph->Modify();
        for (int32 i = Graph->Nodes.Num() - 1; i >= 0; --i)
        {
            if (UEdGraphNode* N = Graph->Nodes[i])
            {
                Graph->RemoveNode(N);
            }
        }

        if (!MergedText.IsEmpty())
        {
            if (!FEdGraphUtilities::CanImportNodesFromText(Graph, MergedText))
            {
                OutError = TEXT("CanImportNodesFromText returned false (incompatible node text for this graph)");
                return false;
            }
            TSet<UEdGraphNode*> Imported;
            FEdGraphUtilities::ImportNodesFromText(Graph, MergedText, /*out*/ Imported);
            if (Imported.Num() == 0)
            {
                OutError = TEXT("ImportNodesFromText produced 0 nodes");
                return false;
            }
        }
        Graph->NotifyGraphChanged();
        return true;
    }

    // Additive merge: keep the existing (ours) graph, remove the listed nodes,
    // then PASTE the provided nodes in (UE's supported copy-paste path, no
    // clear) so the importer doesn't crash the way clear+reimport-all does.
    bool PasteNodes(UEdGraph* Graph, const FString& PasteText, const TArray<FString>& RemoveGuids, FString& OutError)
    {
        Graph->Modify();

        if (RemoveGuids.Num() > 0)
        {
            TSet<FString> ToRemove;
            for (const FString& G : RemoveGuids) { ToRemove.Add(G.ToUpper()); }
            for (int32 i = Graph->Nodes.Num() - 1; i >= 0; --i)
            {
                UEdGraphNode* N = Graph->Nodes[i];
                if (N && ToRemove.Contains(N->NodeGuid.ToString(EGuidFormats::Digits).ToUpper()))
                {
                    Graph->RemoveNode(N);
                }
            }
        }

        if (!PasteText.IsEmpty())
        {
            if (!FEdGraphUtilities::CanImportNodesFromText(Graph, PasteText))
            {
                OutError = TEXT("CanImportNodesFromText returned false for paste text");
                return false;
            }
            TSet<UEdGraphNode*> Imported;
            FEdGraphUtilities::ImportNodesFromText(Graph, PasteText, /*out*/ Imported);
        }
        Graph->NotifyGraphChanged();
        return true;
    }
}

// Request fields:
//   targetPath: string  — disk path of a .uasset that lives INSIDE the currently
//                         open project's Content tree. We load it by its /Game
//                         package name so its parent class and references fully
//                         resolve and the saved asset keeps the correct internal
//                         package name.
//   mergedGraphs: { graphName: nodeText }
//   outPath (optional): explicit destination .uasset to write; defaults to a
//                         temp file under the project's Intermediate dir.
// Response: { ok, mergedPath } | { ok:false, error }
void FMergeApplier::Apply(const TSharedPtr<FJsonObject>& Req, TSharedRef<FJsonObject>& OutResponse)
{
    FString TargetPath;
    if (!Req->TryGetStringField(TEXT("targetPath"), TargetPath))
    {
        // Back-compat: older callers used "path".
        if (!Req->TryGetStringField(TEXT("path"), TargetPath))
        {
            OutResponse->SetBoolField(TEXT("ok"), false);
            OutResponse->SetStringField(TEXT("error"), TEXT("missing 'targetPath'"));
            return;
        }
    }

    // Two modes:
    //  - additiveGraphs: { name: { paste: text, remove: [guid,...] } }  (preferred)
    //  - mergedGraphs:   { name: fullText }  (legacy clear+reimport)
    const TSharedPtr<FJsonObject>* MergedGraphsObj = nullptr;
    const TSharedPtr<FJsonObject>* AdditiveObj = nullptr;
    const bool bHasMerged = Req->TryGetObjectField(TEXT("mergedGraphs"), MergedGraphsObj) && MergedGraphsObj;
    const bool bHasAdditive = Req->TryGetObjectField(TEXT("additiveGraphs"), AdditiveObj) && AdditiveObj;
    if (!bHasMerged && !bHasAdditive)
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"), TEXT("missing 'additiveGraphs' or 'mergedGraphs'"));
        return;
    }

    // Map the on-disk target to its /Game long package name. This only succeeds
    // when the file is inside the open project's mounted Content tree.
    FString PackageName;
    if (!FPackageName::TryConvertFilenameToLongPackageName(TargetPath, PackageName))
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"),
            FString::Printf(TEXT("target is not inside the open project's Content: %s"), *TargetPath));
        return;
    }

    UPackage* Package = LoadPackage(nullptr, *PackageName, LOAD_None);
    if (!Package)
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"),
            FString::Printf(TEXT("LoadPackage failed for %s"), *PackageName));
        return;
    }
    Package->FullyLoad();

    UBlueprint* BP = FindBlueprintInPackage(Package);
    if (!BP)
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"),
            FString::Printf(TEXT("no Blueprint found in package %s"), *PackageName));
        return;
    }

    FString Err;
    if (bHasAdditive)
    {
        for (const auto& Kv : (*AdditiveObj)->Values)
        {
            const FString& GraphName = Kv.Key;
            const TSharedPtr<FJsonObject>* Spec = nullptr;
            if (!Kv.Value.IsValid() || !Kv.Value->TryGetObject(Spec) || !Spec) { continue; }
            FString Paste;
            (*Spec)->TryGetStringField(TEXT("paste"), Paste);
            TArray<FString> Remove;
            const TArray<TSharedPtr<FJsonValue>>* RemArr = nullptr;
            if ((*Spec)->TryGetArrayField(TEXT("remove"), RemArr) && RemArr)
            {
                for (const TSharedPtr<FJsonValue>& V : *RemArr) { FString S; if (V->TryGetString(S)) { Remove.Add(S); } }
            }
            UEdGraph* Graph = FindGraphByName(BP, GraphName);
            if (!Graph)
            {
                OutResponse->SetBoolField(TEXT("ok"), false);
                OutResponse->SetStringField(TEXT("error"), FString::Printf(TEXT("graph '%s' not found"), *GraphName));
                return;
            }
            if (!PasteNodes(Graph, Paste, Remove, Err))
            {
                OutResponse->SetBoolField(TEXT("ok"), false);
                OutResponse->SetStringField(TEXT("error"), FString::Printf(TEXT("graph '%s': %s"), *GraphName, *Err));
                return;
            }
        }
    }
    else
    {
        for (const auto& Kv : (*MergedGraphsObj)->Values)
        {
            const FString& GraphName = Kv.Key;
            FString MergedText;
            if (!Kv.Value.IsValid() || !Kv.Value->TryGetString(MergedText)) { continue; }
            UEdGraph* Graph = FindGraphByName(BP, GraphName);
            if (!Graph)
            {
                OutResponse->SetBoolField(TEXT("ok"), false);
                OutResponse->SetStringField(TEXT("error"),
                    FString::Printf(TEXT("graph '%s' not found on Blueprint"), *GraphName));
                return;
            }
            if (!ReplaceGraphNodes(Graph, MergedText, Err))
            {
                OutResponse->SetBoolField(TEXT("ok"), false);
                OutResponse->SetStringField(TEXT("error"),
                    FString::Printf(TEXT("graph '%s': %s"), *GraphName, *Err));
                return;
            }
        }
    }

    // Recompile so the generated class matches the new graphs.
    FKismetEditorUtilities::CompileBlueprint(BP, EBlueprintCompileOptions::SkipGarbageCollection);

    // Choose an output path: explicit override, else a temp file under Intermediate.
    FString OutPath;
    if (!Req->TryGetStringField(TEXT("outPath"), OutPath) || OutPath.IsEmpty())
    {
        const FString IntermediateDir = FPaths::ProjectIntermediateDir() / TEXT("UnrealMerge");
        IFileManager::Get().MakeDirectory(*IntermediateDir, /*Tree=*/true);
        const FString UniqueId = FGuid::NewGuid().ToString(EGuidFormats::Short);
        OutPath = IntermediateDir / FString::Printf(TEXT("Merged_%s.uasset"), *UniqueId);
    }

    // Save the package (keeps its /Game internal name) to the chosen file.
    FSavePackageArgs SaveArgs;
    SaveArgs.TopLevelFlags = RF_Public | RF_Standalone;
    SaveArgs.SaveFlags = SAVE_NoError;
    SaveArgs.Error = GError;
    const bool bSaved = UPackage::SavePackage(Package, nullptr, *OutPath, SaveArgs);
    if (!bSaved)
    {
        OutResponse->SetBoolField(TEXT("ok"), false);
        OutResponse->SetStringField(TEXT("error"), FString::Printf(TEXT("SavePackage failed for %s"), *OutPath));
        return;
    }

    OutResponse->SetBoolField(TEXT("ok"), true);
    OutResponse->SetStringField(TEXT("mergedPath"), OutPath.Replace(TEXT("\\"), TEXT("/")));
}
