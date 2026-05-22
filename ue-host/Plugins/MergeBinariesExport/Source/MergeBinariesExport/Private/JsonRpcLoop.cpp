#include "JsonRpcLoop.h"

#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

#include <iostream>
#include <string>

namespace
{
    bool ReadLine(FString& OutLine)
    {
        std::string Line;
        if (!std::getline(std::cin, Line))
        {
            return false;
        }
        // Strip a trailing CR if a Windows tool wrote CRLF.
        if (!Line.empty() && Line.back() == '\r')
        {
            Line.pop_back();
        }
        OutLine = FString(UTF8_TO_TCHAR(Line.c_str()));
        return true;
    }

    void WriteRawJsonLine(const FString& Json)
    {
        const FTCHARToUTF8 Conv(*Json);
        std::cout.write(Conv.Get(), Conv.Length());
        std::cout.put('\n');
        std::cout.flush();
    }
}

void FJsonRpcLoop::WriteResponse(const TSharedRef<FJsonObject>& Response)
{
    FString Out;
    const TSharedRef<TJsonWriter<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>> Writer =
        TJsonWriterFactory<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>::Create(&Out);
    FJsonSerializer::Serialize(Response, Writer);
    WriteRawJsonLine(Out);
}

int32 FJsonRpcLoop::Run(const TMap<FString, FHandler>& Handlers)
{
    FString Line;
    while (ReadLine(Line))
    {
        if (Line.IsEmpty())
        {
            continue;
        }

        TSharedPtr<FJsonObject> Request;
        const TSharedRef<TJsonReader<TCHAR>> Reader = TJsonReaderFactory<TCHAR>::Create(Line);
        if (!FJsonSerializer::Deserialize(Reader, Request) || !Request.IsValid())
        {
            // Non-JSON line on stdin: the parent shouldn't have sent it. Surface and continue.
            const TSharedRef<FJsonObject> Err = MakeShared<FJsonObject>();
            Err->SetBoolField(TEXT("ok"), false);
            Err->SetStringField(TEXT("error"), TEXT("invalid JSON on stdin"));
            WriteResponse(Err);
            continue;
        }

        double IdNum = 0.0;
        const bool bHasId = Request->TryGetNumberField(TEXT("id"), IdNum);

        FString Cmd;
        if (!Request->TryGetStringField(TEXT("cmd"), Cmd))
        {
            const TSharedRef<FJsonObject> Err = MakeShared<FJsonObject>();
            if (bHasId) { Err->SetNumberField(TEXT("id"), IdNum); }
            Err->SetBoolField(TEXT("ok"), false);
            Err->SetStringField(TEXT("error"), TEXT("missing 'cmd' field"));
            WriteResponse(Err);
            continue;
        }

        if (Cmd == TEXT("quit"))
        {
            const TSharedRef<FJsonObject> Ok = MakeShared<FJsonObject>();
            if (bHasId) { Ok->SetNumberField(TEXT("id"), IdNum); }
            Ok->SetBoolField(TEXT("ok"), true);
            WriteResponse(Ok);
            return 0;
        }

        const FHandler* Handler = Handlers.Find(Cmd);
        if (!Handler)
        {
            const TSharedRef<FJsonObject> Err = MakeShared<FJsonObject>();
            if (bHasId) { Err->SetNumberField(TEXT("id"), IdNum); }
            Err->SetBoolField(TEXT("ok"), false);
            Err->SetStringField(TEXT("error"), FString::Printf(TEXT("unknown cmd: %s"), *Cmd));
            WriteResponse(Err);
            continue;
        }

        TSharedRef<FJsonObject> Response = MakeShared<FJsonObject>();
        if (bHasId) { Response->SetNumberField(TEXT("id"), IdNum); }
        (*Handler)(Request, Response);
        WriteResponse(Response);
    }
    return 0;
}
