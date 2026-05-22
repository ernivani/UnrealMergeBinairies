#pragma once

#include "CoreMinimal.h"
#include "Dom/JsonObject.h"

class FJsonRpcLoop
{
public:
    /** Handler signature: receives the request JSON, must populate `OutResponse` with success/failure payload. */
    using FHandler = TFunction<void(const TSharedPtr<FJsonObject>& Request, TSharedRef<FJsonObject>& OutResponse)>;

    /** Run the loop until stdin closes or a `quit` command arrives. Returns the exit code to bubble to UCommandlet. */
    static int32 Run(const TMap<FString, FHandler>& Handlers);

    /** Write a single response object to stdout as one `\n`-terminated UTF-8 JSON line. */
    static void WriteResponse(const TSharedRef<FJsonObject>& Response);
};
