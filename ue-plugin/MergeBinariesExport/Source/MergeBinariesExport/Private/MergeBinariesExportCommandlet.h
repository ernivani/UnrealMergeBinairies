#pragma once

#include "CoreMinimal.h"
#include "Commandlets/Commandlet.h"
#include "MergeBinariesExportCommandlet.generated.h"

UCLASS()
class UMergeBinariesExportCommandlet : public UCommandlet
{
    GENERATED_BODY()

public:
    UMergeBinariesExportCommandlet();

    virtual int32 Main(const FString& Params) override;
};
