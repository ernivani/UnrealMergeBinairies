#include "MergeBinariesExportCommandlet.h"
#include "AssetExporter.h"
#include "JsonRpcLoop.h"

DEFINE_LOG_CATEGORY_STATIC(LogMergeBinariesExport, Log, All);

UMergeBinariesExportCommandlet::UMergeBinariesExportCommandlet()
{
    IsClient = false;
    IsEditor = true;
    IsServer = false;
    LogToConsole = false;
    ShowErrorCount = false;
    ShowProgress = false;
}

int32 UMergeBinariesExportCommandlet::Main(const FString& Params)
{
    UE_LOG(LogMergeBinariesExport, Display, TEXT("MergeBinariesExport commandlet started; entering JSON-RPC loop"));

    TMap<FString, FJsonRpcLoop::FHandler> Handlers;

    Handlers.Add(TEXT("ping"), [](const TSharedPtr<FJsonObject>& /*Req*/, TSharedRef<FJsonObject>& OutResponse)
    {
        OutResponse->SetBoolField(TEXT("ok"), true);
        OutResponse->SetStringField(TEXT("pong"), TEXT("MergeBinariesExport"));
    });

    Handlers.Add(TEXT("export"), [](const TSharedPtr<FJsonObject>& Req, TSharedRef<FJsonObject>& OutResponse)
    {
        FString Path;
        if (!Req->TryGetStringField(TEXT("path"), Path))
        {
            OutResponse->SetBoolField(TEXT("ok"), false);
            OutResponse->SetStringField(TEXT("error"), TEXT("missing 'path' field"));
            return;
        }
        FAssetExporter::Export(Path, OutResponse);
    });

    return FJsonRpcLoop::Run(Handlers);
}
