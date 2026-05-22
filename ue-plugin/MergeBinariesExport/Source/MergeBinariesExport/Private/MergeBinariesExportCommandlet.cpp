#include "MergeBinariesExportCommandlet.h"

DEFINE_LOG_CATEGORY_STATIC(LogMergeBinariesExport, Log, All);

UMergeBinariesExportCommandlet::UMergeBinariesExportCommandlet()
{
    IsClient = false;
    IsEditor = true;
    IsServer = false;
    LogToConsole = false;       // keep our stdout clean for JSON only
    ShowErrorCount = false;
    ShowProgress = false;
}

int32 UMergeBinariesExportCommandlet::Main(const FString& Params)
{
    UE_LOG(LogMergeBinariesExport, Display, TEXT("MergeBinariesExport commandlet starting (stub)"));
    // Real work in Task 3.
    return 0;
}
