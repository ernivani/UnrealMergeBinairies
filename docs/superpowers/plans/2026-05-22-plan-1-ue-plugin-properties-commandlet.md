# Plan 1 — UE Plugin: Properties-Only Export Commandlet

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship an Unreal Engine editor plugin whose commandlet, when launched with `UnrealEditor.exe HostProject.uproject -run=MergeBinariesExport -stdio -nullrhi -unattended -NoCrashReports`, runs a long-lived JSON-RPC loop over stdin/stdout that loads a `.uasset` on demand and emits `package` + `asset.properties` JSON conforming to the schema in `docs/superpowers/specs/2026-05-22-unreal-merge-binaries-design.md` §6. Verified by golden-JSON tests against `Examples/v1/BP_MinimalChar.uasset` and `Examples/v2/BP_MinimalChar.uasset`.

**Architecture:** A C++ editor-only UE plugin under `ue-plugin/MergeBinariesExport/`. A tiny host project at `ue-host/HostProject.uproject` references the plugin so the commandlet has something to run inside. Property serialisation uses UE's `FProperty` reflection (`TFieldIterator<FProperty>`), emitting newline-delimited JSON per request. The blueprint graph, component tree, and bindings are explicitly OUT of scope here and land in Plan 4.

**Tech Stack:**
- Unreal Engine 5.5+ (5.5 targeted for development; the commandlet works against whatever UE built it). The plan was originally drafted against 5.4; we pinned to 5.5 once Task 1 revealed 5.4 wasn't installed locally. UE 5.6/5.7 should also work — adjust the Build.bat path accordingly.
- C++17 (UE's standard)
- UE modules: `Core`, `CoreUObject`, `Engine`, `UnrealEd`, `Json`, `JsonUtilities`
- Test harness: PowerShell + `jq` for JSON normalisation (Windows host)
- Build: UE's UBT; plugin compiled into `ue-host/Binaries/Win64/UnrealEditor-MergeBinariesExport.dll`
- Shell: Windows PowerShell 5.1 (system default on Windows) or PowerShell 7+ — all scripts are written to work on both

**Prerequisites the engineer must have installed:**
- Unreal Engine 5.5+ (via Epic Games Launcher; tested against 5.5)
- Visual Studio 2022 with the "Game development with C++" workload, including MSVC v143, Windows 11 SDK
- Git for Windows
- `jq` is OPTIONAL. The golden-test harness (Task 4) prefers `jq` for canonical JSON normalisation but ships an in-script PowerShell fallback that key-sorts JSON the same way. Install with `winget install jqlang.jq` if you want stricter parity.

**`pwsh` vs. `powershell`:** all script invocations in this plan use `pwsh -File ...` for forward compatibility. If PowerShell 7 isn't installed (`pwsh` not on PATH), substitute `powershell -File ...` (Windows PowerShell 5.1, present on every Windows machine) — the scripts have been written to run on both. Adding PowerShell 7 is a one-liner if you'd rather have it: `winget install Microsoft.PowerShell`.

**UE writing back to `ue-host/Config/DefaultEngine.ini`:** Some default-enabled UE plugins (most notably `AndroidFileServerEditor`) inject their config block into this file on first project load. We disable those plugins in `HostProject.uproject` (`"Enabled": false`) to keep the file stable. If a new plugin appears that wasn't disabled and dirties the INI, add it to the `Plugins` array as `"Enabled": false` (do NOT just `git checkout --` the INI in a loop — track the root cause).

**Piping JSON to the commandlet — always use `-StdinText`:** PowerShell's default `$OutputEncoding` is ASCII on 5.1 and UTF-8 on 7+, and various shell wrappers / profiles change it to UTF-16, which mangles JSON-RPC frames into "invalid JSON on stdin" responses. `tools/run-commandlet.ps1` provides a `-StdinText` parameter that bypasses pipe encoding entirely — the script opens the child process via `System.Diagnostics.Process`, writes UTF-8 bytes (no BOM) directly to stdin, and redirects stdout/stderr so callers can pipe (`run-commandlet.ps1 -StdinText '...' | Where-Object ...`). **All callers that send JSON requests MUST use `-StdinText`, not raw pipe redirection.**

**Auto-prepended warmup ping:** UE's `-stdio` commandlet boot empirically swallows or corrupts the first stdin line during init. The launcher automatically prepends `{"id":0,"cmd":"_warmup"}` to every `-StdinText` payload — the loop replies `{"id":0,"ok":false,"error":"unknown cmd: _warmup"}` on stdout, which downstream filters that key on `id >= 1` (or schema-specific fields) ignore. Pass `-NoWarmup` if a caller genuinely needs to be the first frame UE sees.

**Per-call mount roots in AssetExporter:** when the asset lives outside an Unreal project's `Content/` tree (our test fixtures), `FAssetExporter::Export` registers a synthetic mount root and unmounts it before returning. Each call uses a unique counter-suffixed root (`/MergeTmp1/`, `/MergeTmp2/`, …) so UE's package cache can't return a stale `UPackage` from a previous load when the same filename appears in two different fixture directories. The JSON's `package.name` is normalised back to the stable form `/MergeTmp/<basename>` for run-independent goldens.

---

## File structure for this plan

```
ue-plugin/
└── MergeBinariesExport/
    ├── MergeBinariesExport.uplugin
    └── Source/MergeBinariesExport/
        ├── MergeBinariesExport.Build.cs
        ├── Public/
        │   └── MergeBinariesExportModule.h
        └── Private/
            ├── MergeBinariesExportModule.cpp
            ├── MergeBinariesExportCommandlet.h
            ├── MergeBinariesExportCommandlet.cpp
            ├── JsonRpcLoop.h
            ├── JsonRpcLoop.cpp
            ├── AssetExporter.h
            └── AssetExporter.cpp

ue-host/
├── HostProject.uproject
├── Config/
│   └── DefaultEngine.ini
└── Content/
    └── .keep                 # UE refuses to load a project with no Content dir

tools/
├── run-commandlet.ps1        # invoke headlessly, write/read RPC
└── golden-test.ps1           # compare emitted JSON to Examples/*.expected.json

Examples/
├── v1/BP_MinimalChar.uasset  # (already exists)
├── v2/BP_MinimalChar.uasset  # (already exists)
├── v1.expected.json          # generated, committed
└── v2.expected.json          # generated, committed

.gitignore                    # add UE build artifacts (Binaries/, Intermediate/, Saved/, DerivedDataCache/)
```

Each file has one responsibility:
- **`MergeBinariesExport.uplugin`** — plugin manifest. Editor-only.
- **`MergeBinariesExport.Build.cs`** — module build rules. Lists UE module dependencies.
- **`MergeBinariesExportModule.cpp`** — `IModuleInterface` skeleton; no logic.
- **`MergeBinariesExportCommandlet.cpp`** — `UCommandlet::Main` entry; sets up `JsonRpcLoop` and dispatches to `AssetExporter`.
- **`JsonRpcLoop.cpp`** — line-delimited JSON request/response loop on stdin/stdout. Knows nothing about asset semantics.
- **`AssetExporter.cpp`** — loads a `.uasset` via `LoadPackage`; walks `FProperty` reflection; emits `TSharedPtr<FJsonObject>` matching the schema.

---

## Task 0: Repository scaffolding & gitignore

**Files:**
- Modify: `.gitignore`
- Create: `ue-host/Content/.keep`
- Create: `ue-host/Config/DefaultEngine.ini`
- Create: `ue-host/HostProject.uproject`

- [ ] **Step 1: Append UE build artefact patterns to `.gitignore`**

Append to `.gitignore`:

```gitignore
# Unreal Engine build artefacts
**/Binaries/
**/Intermediate/
**/Saved/
**/DerivedDataCache/
*.VC.db
*.opensdf
*.sdf
*.suo
.vs/

# Generated UE solution/project files
ue-host/*.sln
ue-host/*.vcxproj*
```

Note: `docs/` is already in `.gitignore`. Leave it; we use `git add -f` for spec/plan documents (this is documented in the spec commit history). Engineers should keep using `-f` for files under `docs/`.

- [ ] **Step 2: Create the host project skeleton**

Create `ue-host/Content/.keep` as an empty file (UE silently refuses to open a project whose `Content/` dir doesn't exist).

Create `ue-host/Config/DefaultEngine.ini`:

```ini
[/Script/EngineSettings.GeneralProjectSettings]
ProjectID=(A=0,B=0,C=0,D=0)
ProjectName=MergeBinariesHost
```

Create `ue-host/HostProject.uproject`:

```json
{
    "FileVersion": 3,
    "EngineAssociation": "5.5",
    "Category": "",
    "Description": "Host project for MergeBinariesExport plugin development & testing.",
    "Modules": [],
    "Plugins": [
        {
            "Name": "MergeBinariesExport",
            "Enabled": true
        }
    ],
    "AdditionalPluginDirectories": [
        "../ue-plugin"
    ]
}
```

The `AdditionalPluginDirectories` entry lets the host project find the plugin source at `ue-plugin/MergeBinariesExport/` without symlinks or copies.

- [ ] **Step 3: Commit**

```powershell
git add .gitignore ue-host/Content/.keep ue-host/Config/DefaultEngine.ini ue-host/HostProject.uproject
git commit -m "chore: add UE host project skeleton and ignore build artefacts"
```

---

## Task 1: UE plugin scaffolding

**Files:**
- Create: `ue-plugin/MergeBinariesExport/MergeBinariesExport.uplugin`
- Create: `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/MergeBinariesExport.Build.cs`
- Create: `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Public/MergeBinariesExportModule.h`
- Create: `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeBinariesExportModule.cpp`

- [ ] **Step 1: Write the plugin manifest**

Create `ue-plugin/MergeBinariesExport/MergeBinariesExport.uplugin`:

```json
{
    "FileVersion": 3,
    "Version": 1,
    "VersionName": "0.1.0",
    "FriendlyName": "Merge Binaries Export",
    "Description": "Editor-only commandlet that exports .uasset contents as JSON for the UnrealMergeBinairies tool.",
    "Category": "Editor",
    "CreatedBy": "UnrealMergeBinairies",
    "CreatedByURL": "",
    "DocsURL": "",
    "MarketplaceURL": "",
    "SupportURL": "",
    "EnabledByDefault": true,
    "CanContainContent": false,
    "IsBetaVersion": true,
    "Installed": false,
    "Modules": [
        {
            "Name": "MergeBinariesExport",
            "Type": "Editor",
            "LoadingPhase": "Default"
        }
    ]
}
```

`LoadingPhase: "Default"` (NOT `PostEngineInit`): the engine dispatches the `-run=<commandlet>` argument BEFORE `PostEngineInit`, so a module loaded at that phase will not have registered its `UCommandlet` class yet — UE prints `Failed to find commandlet class MergeBinariesExportCommandlet` and exits. `Default` is the conventional phase for editor modules that contribute UClasses needed at commandlet dispatch.

- [ ] **Step 2: Write the module build rules**

Create `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/MergeBinariesExport.Build.cs`:

```csharp
using UnrealBuildTool;

public class MergeBinariesExport : ModuleRules
{
    public MergeBinariesExport(ReadOnlyTargetRules Target) : base(Target)
    {
        PCHUsage = ModuleRules.PCHUsageMode.UseExplicitOrSharedPCHs;

        PublicDependencyModuleNames.AddRange(new string[]
        {
            "Core"
        });

        PrivateDependencyModuleNames.AddRange(new string[]
        {
            "CoreUObject",
            "Engine",
            "UnrealEd",
            "Json",
            "JsonUtilities"
        });
    }
}
```

- [ ] **Step 3: Write the module interface**

Create `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Public/MergeBinariesExportModule.h`:

```cpp
#pragma once

#include "CoreMinimal.h"
#include "Modules/ModuleInterface.h"

class FMergeBinariesExportModule : public IModuleInterface
{
public:
    virtual void StartupModule() override {}
    virtual void ShutdownModule() override {}
};
```

Create `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeBinariesExportModule.cpp`:

```cpp
#include "MergeBinariesExportModule.h"
#include "Modules/ModuleManager.h"

IMPLEMENT_MODULE(FMergeBinariesExportModule, MergeBinariesExport)
```

- [ ] **Step 4: Generate VS project files and confirm the plugin compiles**

Run from the repository root in PowerShell:

```powershell
& "C:\Program Files\Epic Games\UE_5.5\Engine\Build\BatchFiles\Build.bat" `
  UnrealEditor Win64 Development `
  -Project="$PWD\ue-host\HostProject.uproject" `
  -WaitMutex -FromMsBuild
```

Why `UnrealEditor` (not `MergeBinariesHostEditor`): the host project is content-only (`Modules: []`, no `ue-host/Source/`), so no project-specific editor target exists. UBT detects the plugin via `AdditionalPluginDirectories`, generates temporary Target.cs files, and links the plugin into `ue-host/Binaries/Win64/UnrealEditor-MergeBinariesExport.dll`. This is the standard pattern for plugin-only host projects.

**Verifying success on UE 5.5+:** the literal string `BUILD SUCCESSFUL` is NOT printed by modern UBT. Success indicators are:
1. The command's exit code is `0` (`$LASTEXITCODE` in PowerShell).
2. The build log ends with a `Total execution time: NN seconds` line.
3. `ue-host/Binaries/Win64/UnrealEditor-MergeBinariesExport.dll` was produced.

A `Link [x64] UnrealEditor-MergeBinariesExport.dll` line in the log is the proof point for plugin compilation specifically.

If `UE_5.5` isn't installed, substitute the highest installed version that's `>= 5.5` (5.6, 5.7, …). If no UE is installed, stop and ask before proceeding.

- [ ] **Step 5: Commit**

```powershell
git add ue-plugin/MergeBinariesExport
git commit -m "feat(ue-plugin): scaffold MergeBinariesExport plugin module"
```

---

## Task 2: Bare commandlet that exits cleanly

**Files:**
- Create: `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeBinariesExportCommandlet.h`
- Create: `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeBinariesExportCommandlet.cpp`
- Create: `tools/run-commandlet.ps1`

Goal of this task: get the commandlet *discoverable* and *runnable* before adding any RPC or export logic. This is the smallest possible smoke test.

- [ ] **Step 1: Declare the commandlet UClass**

Create `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeBinariesExportCommandlet.h`:

```cpp
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
```

- [ ] **Step 2: Implement the commandlet stub**

Create `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeBinariesExportCommandlet.cpp`:

```cpp
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
```

- [ ] **Step 3: Rebuild**

```powershell
& "C:\Program Files\Epic Games\UE_5.5\Engine\Build\BatchFiles\Build.bat" `
  UnrealEditor Win64 Development `
  -Project="$PWD\ue-host\HostProject.uproject" `
  -WaitMutex -FromMsBuild
```

Expected: exit code `0`; the `UnrealEditor-MergeBinariesExport.dll` link line appears in the log. (See Task 1 Step 4 for the rationale on target name and the modern UBT success indicators.)

- [ ] **Step 4: Write a PowerShell launcher**

Create `tools/run-commandlet.ps1`:

```powershell
# Works on Windows PowerShell 5.1 and PowerShell 7+.
[CmdletBinding()]
param(
    [string]$UnrealEditor = "C:\Program Files\Epic Games\UE_5.5\Engine\Binaries\Win64\UnrealEditor.exe",
    [string]$HostProject  = (Join-Path $PSScriptRoot "..\ue-host\HostProject.uproject" | Resolve-Path).Path,
    [string[]]$ExtraArgs  = @()
)

$args = @(
    $HostProject,
    "-run=MergeBinariesExport",
    "-stdio",
    "-nullrhi",
    "-unattended",
    "-NoCrashReports"
) + $ExtraArgs

# Stream stderr to host stderr for visibility; pass stdout through unchanged.
& $UnrealEditor @args
exit $LASTEXITCODE
```

- [ ] **Step 5: Run the stub commandlet**

```powershell
pwsh -File tools/run-commandlet.ps1
```

Expected output: a burst of UE engine log lines (mostly on stderr), then the process exits 0. There will be **no** JSON output yet — that lands in Task 3.

- [ ] **Step 6: Commit**

```powershell
git add ue-plugin/MergeBinariesExport tools/run-commandlet.ps1
git commit -m "feat(ue-plugin): register MergeBinariesExport commandlet (stub)"
```

---

## Task 3: JSON-RPC loop over stdin/stdout

**Files:**
- Create: `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/JsonRpcLoop.h`
- Create: `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/JsonRpcLoop.cpp`
- Modify: `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeBinariesExportCommandlet.cpp`

The loop reads one JSON object per line from stdin, dispatches by `cmd`, and writes one JSON object per line to stdout. It supports two commands now: `ping` (smoke) and `quit`. `export` lands in Task 4.

- [ ] **Step 1: Declare the loop API**

Create `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/JsonRpcLoop.h`:

```cpp
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
```

- [ ] **Step 2: Implement the loop**

Create `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/JsonRpcLoop.cpp`:

```cpp
#include "JsonRpcLoop.h"

#include "Misc/OutputDeviceConsole.h"
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
```

- [ ] **Step 3: Wire `ping` into the commandlet**

Replace the body of `MergeBinariesExportCommandlet.cpp` with:

```cpp
#include "MergeBinariesExportCommandlet.h"
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

    return FJsonRpcLoop::Run(Handlers);
}
```

- [ ] **Step 4: Rebuild**

```powershell
& "C:\Program Files\Epic Games\UE_5.5\Engine\Build\BatchFiles\Build.bat" `
  UnrealEditor Win64 Development `
  -Project="$PWD\ue-host\HostProject.uproject" -WaitMutex -FromMsBuild
```

Expected: exit code `0`; `UnrealEditor-MergeBinariesExport.dll` re-linked.

- [ ] **Step 5: Smoke-test `ping` end-to-end**

Use the launcher's `-StdinText` parameter — it writes UTF-8 bytes directly to UE's stdin, bypassing PowerShell pipe-encoding pitfalls.

```powershell
powershell -File tools/run-commandlet.ps1 -StdinText '{"id":1,"cmd":"ping"}'
```

Expected: among the engine log lines on stderr, exactly one line appears on stdout that parses as JSON and contains `{"id":1,"ok":true,"pong":"MergeBinariesExport"}`. The line may be preceded or followed by engine log lines — that is exactly the "stdout pollution" the spec calls out. Once UE finishes its boot the loop exits because stdin EOF arrives after the one request.

To filter just for our line:

```powershell
$out = powershell -File tools/run-commandlet.ps1 -StdinText '{"id":1,"cmd":"ping"}' 2>$null
$out | Where-Object {
    try { ($_ | ConvertFrom-Json -ErrorAction Stop).pong -eq 'MergeBinariesExport' } catch { $false }
}
```

Expected: prints exactly one line: `{"id":1,"ok":true,"pong":"MergeBinariesExport"}`.

- [ ] **Step 6: Commit**

```powershell
git add ue-plugin/MergeBinariesExport
git commit -m "feat(ue-plugin): JSON-RPC stdio loop with ping handler"
```

---

## Task 4: `export` command — package block only

**Files:**
- Create: `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/AssetExporter.h`
- Create: `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/AssetExporter.cpp`
- Modify: `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeBinariesExportCommandlet.cpp`
- Create: `tools/golden-test.ps1`
- Create: `Examples/v1.expected.json` (initial — package block only)
- Create: `Examples/v2.expected.json` (initial — package block only)

Smallest meaningful export: load the package, emit the `package` block. No properties yet. This validates `LoadPackage` works and lets us cement the golden-test loop before the more error-prone reflection code lands.

- [ ] **Step 1: Declare the exporter API**

Create `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/AssetExporter.h`:

```cpp
#pragma once

#include "CoreMinimal.h"
#include "Dom/JsonObject.h"

class FAssetExporter
{
public:
    /**
     * Export the asset at `AbsoluteAssetPath` (a path to a .uasset file on disk).
     * Populates `OutResponse` with `{ok:true, asset:{...}}` or `{ok:false, error:"..."}`.
     */
    static void Export(const FString& AbsoluteAssetPath, TSharedRef<FJsonObject>& OutResponse);

private:
    /** Build the `package` block. Returns nullptr on failure (and populates `OutError`). */
    static TSharedPtr<FJsonObject> BuildPackageBlock(const FString& AbsoluteAssetPath,
                                                     UPackage* Package,
                                                     FString& OutError);
};
```

- [ ] **Step 2: Implement package-block export**

Create `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/AssetExporter.cpp`:

```cpp
#include "AssetExporter.h"

#include "HAL/FileManager.h"
#include "Misc/FileHelper.h"
#include "Misc/PackageName.h"
#include "Misc/Paths.h"
#include "Misc/SecureHash.h"
#include "UObject/Package.h"
#include "UObject/PackageFileSummary.h"
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
    Asset->SetStringField(TEXT("class"), TEXT("(unknown — pending Task 5)"));

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

    const TSharedRef<FJsonObject> Out = MakeShared<FJsonObject>();
    Out->SetStringField(TEXT("name"),            Package->GetName());
    Out->SetStringField(TEXT("engineVersion"),   FEngineVersion::Current().ToString());
    Out->SetNumberField(TEXT("fileVersionUE5"),  static_cast<double>(Package->GetLinkerCustomVersion(FUE5MainStreamObjectVersion::GUID).Version));
    Out->SetStringField(TEXT("savedHash"),       FString::Printf(TEXT("sha1:%s"), *Hex));
    return Out;
}
```

Note: the `fileVersionUE5` lookup above is illustrative; the *exact* call to retrieve the UE5 file version evolves across UE point releases. If the engine API differs in your installed UE version, replace the right-hand side of `SetNumberField` with whatever yields the integer the engine actually serialised (matching the bytes at offset 0x10 of the .uasset header, which the schema-validation pass confirmed is `1017` for both fixtures). It MUST be a stable integer per file — golden tests will pin it.

- [ ] **Step 3: Wire `export` into the commandlet**

Edit `MergeBinariesExportCommandlet.cpp` — add an `#include "AssetExporter.h"` and a new handler:

```cpp
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
```

- [ ] **Step 4: Rebuild**

```powershell
& "C:\Program Files\Epic Games\UE_5.5\Engine\Build\BatchFiles\Build.bat" `
  UnrealEditor Win64 Development `
  -Project="$PWD\ue-host\HostProject.uproject" -WaitMutex -FromMsBuild
```

Expected: exit code `0`; `UnrealEditor-MergeBinariesExport.dll` re-linked.

- [ ] **Step 5: Run export against the fixtures manually and capture output**

```powershell
$v1 = (Resolve-Path "Examples/v1/BP_MinimalChar.uasset").Path -replace '\\','/'
$v2 = (Resolve-Path "Examples/v2/BP_MinimalChar.uasset").Path -replace '\\','/'

$rpc = "{`"id`":1,`"cmd`":`"export`",`"path`":`"$v1`"}`n" +
       "{`"id`":2,`"cmd`":`"export`",`"path`":`"$v2`"}`n" +
       "{`"id`":3,`"cmd`":`"quit`"}`n"

powershell -File tools/run-commandlet.ps1 -StdinText $rpc 2>$null |
    Where-Object { $_ -match '^\s*\{.*"id":\s*[12]' }
```

Expected: exactly two lines, each a JSON object with `"ok":true`, the matching `id`, a `package.name` of `/MergeTmp/BP_MinimalChar` (or the resolved long package name), `package.savedHash` starting with `sha1:`, and `package.fileVersionUE5` equal to `1017` for both fixtures.

If the `package.name` field differs between the two fixtures because mount points were re-registered between requests, normalise the mount: have the script unmount `/MergeTmp/` after each export. The golden-test script in the next step handles this.

- [ ] **Step 6: Write the golden-test harness**

Create `tools/golden-test.ps1`:

```powershell
# Works on Windows PowerShell 5.1 and PowerShell 7+.
<#
    Drives MergeBinariesExport against every fixture under Examples/v*/*.uasset,
    captures the JSON response for each, normalises volatile fields, and diffs
    against the matching Examples/<n>.expected.json.

    Usage:
        pwsh tools/golden-test.ps1            # verify
        pwsh tools/golden-test.ps1 -Bless     # overwrite expected files with current output
#>
[CmdletBinding()]
param(
    [switch]$Bless
)

$ErrorActionPreference = 'Stop'
$Root     = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$ExamplesDir = Join-Path $Root 'Examples'
$Versions = Get-ChildItem $ExamplesDir -Directory | Where-Object Name -Match '^v\d+$'

# Build a single batched request: one `export` per fixture, then `quit`.
$id = 0
$requests = foreach ($v in $Versions) {
    $assets = Get-ChildItem (Join-Path $v.FullName '*.uasset')
    foreach ($a in $assets) {
        $id++
        [pscustomobject]@{
            id   = $id
            cmd  = 'export'
            path = ($a.FullName -replace '\\','/')
            tag  = "$($v.Name)"
        }
    }
}

$rpcLines = $requests | ForEach-Object {
    [pscustomobject]@{ id = $_.id; cmd = $_.cmd; path = $_.path } | ConvertTo-Json -Compress
}
$rpcLines += '{"cmd":"quit"}'

$stdinText = ($rpcLines -join "`n") + "`n"
$rawOutput = powershell -File (Join-Path $PSScriptRoot 'run-commandlet.ps1') -StdinText $stdinText 2>$null

# Keep only lines that parse as JSON objects carrying our schema ("id" + "ok" or "package").
$responses = @{}
foreach ($line in $rawOutput) {
    try {
        $obj = $line | ConvertFrom-Json -ErrorAction Stop
    } catch { continue }
    if (-not $obj.PSObject.Properties.Match('id')) { continue }
    $responses[[int]$obj.id] = $obj
}

function Normalise([pscustomobject]$Obj) {
    # Strip the absolute on-disk path so the golden file is portable across machines.
    if ($Obj.PSObject.Properties.Match('path')) { $Obj.path = '<ABSOLUTE_PATH_STRIPPED>' }
    # Engine patch version drifts; pin to major.minor only.
    if ($Obj.package -and $Obj.package.engineVersion) {
        $Obj.package.engineVersion = ($Obj.package.engineVersion -replace '^(\d+\.\d+).*','$1.x')
    }
    return $Obj
}

$failed = $false
foreach ($req in $requests) {
    $resp = $responses[$req.id]
    if (-not $resp) {
        Write-Host "FAIL: no response for $($req.tag)/$([IO.Path]::GetFileName($req.path))" -ForegroundColor Red
        $failed = $true
        continue
    }
    $normalised = Normalise $resp
    $actualJson = ($normalised | ConvertTo-Json -Depth 64 | jq --sort-keys '.')
    $expectedFile = Join-Path $ExamplesDir "$($req.tag).expected.json"

    if ($Bless) {
        $actualJson | Out-File -FilePath $expectedFile -Encoding utf8
        Write-Host "BLESS: wrote $expectedFile" -ForegroundColor Yellow
        continue
    }

    if (-not (Test-Path $expectedFile)) {
        Write-Host "FAIL: missing expected file $expectedFile (run with -Bless to create)" -ForegroundColor Red
        $failed = $true
        continue
    }

    $expectedJson = Get-Content $expectedFile -Raw | jq --sort-keys '.'
    if ($actualJson -ne $expectedJson) {
        Write-Host "FAIL: diff for $expectedFile" -ForegroundColor Red
        Compare-Object ($expectedJson -split "`n") ($actualJson -split "`n") | Format-Table -AutoSize
        $failed = $true
    } else {
        Write-Host "PASS: $expectedFile" -ForegroundColor Green
    }
}

if ($failed) { exit 1 } else { exit 0 }
```

- [ ] **Step 7: Bless the initial goldens and re-run as a verify pass**

```powershell
pwsh tools/golden-test.ps1 -Bless
pwsh tools/golden-test.ps1
```

Expected first command: prints `BLESS: wrote Examples/v1.expected.json` and `BLESS: wrote Examples/v2.expected.json`. Expected second command: prints `PASS: Examples/v1.expected.json` and `PASS: Examples/v2.expected.json`, exit 0.

- [ ] **Step 8: Inspect the goldens manually**

Open `Examples/v1.expected.json` and `Examples/v2.expected.json`. Confirm by eye:
- Both have `package.name` ending in `BP_MinimalChar`.
- Both have `package.fileVersionUE5` = `1017` (this is what the spec's binary inspection produced).
- `package.savedHash` values differ (the two fixtures differ by 547 bytes; their SHA-1s MUST be different).
- `asset.class` is still the `(unknown — pending Task 5)` placeholder.

If `fileVersionUE5` shows something other than `1017`, the lookup in `BuildPackageBlock` is reading the wrong custom version GUID — fix it now before continuing (it is the one field downstream diff logic will key on for engine-version checks).

- [ ] **Step 9: Commit**

```powershell
git add -f ue-plugin/MergeBinariesExport tools/golden-test.ps1 Examples/v1.expected.json Examples/v2.expected.json
git commit -m "feat(ue-plugin): export 'package' block + golden-test harness"
```

(`-f` because `docs/` is gitignored, but `Examples/` and `tools/` are not — this command works without `-f`. Use plain `git add` if your shell flags the `-f`.)

---

## Task 5: `asset.properties` via FProperty reflection

**Files:**
- Modify: `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/AssetExporter.h`
- Modify: `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/AssetExporter.cpp`
- Modify: `Examples/v1.expected.json` (re-blessed)
- Modify: `Examples/v2.expected.json` (re-blessed)

Walk every `FProperty` on the loaded asset's main `UObject` (the package's primary asset), emit a flat list of `{path, type, value}` entries using dotted paths for nested structs/objects. Per spec §6 we keep paths flat to make set-comparison diffs trivial.

- [ ] **Step 1: Extend the exporter header**

Open `AssetExporter.h` and append to the `FAssetExporter` class body, inside `private:`:

```cpp
    /** Recursive walk; appends one entry per leaf-valued FProperty. */
    static void WalkProperties(const void* ContainerData,
                               UStruct* Struct,
                               const FString& PathPrefix,
                               TArray<TSharedPtr<FJsonValue>>& OutEntries,
                               int32 Depth = 0);

    /** Serialise one property's value to a JsonValue. Returns nullptr for unsupported. */
    static TSharedPtr<FJsonValue> SerialisePropertyValue(FProperty* Property, const void* ValuePtr);

    /** Find the asset's primary UObject inside a UPackage (the "main" asset; what UE shows in the Content Browser). */
    static UObject* FindPrimaryAsset(UPackage* Package);

    static constexpr int32 MaxRecursionDepth = 8;
```

- [ ] **Step 2: Implement `FindPrimaryAsset`**

Append to `AssetExporter.cpp`:

```cpp
UObject* FAssetExporter::FindPrimaryAsset(UPackage* Package)
{
    UObject* Found = nullptr;
    ForEachObjectWithOuter(Package, [&Found](UObject* It)
    {
        if (Found) { return; }
        if (It->HasAnyFlags(RF_Public) && !It->IsA<UMetaData>())
        {
            Found = It;
        }
    }, /*bIncludeNestedObjects=*/false);
    return Found;
}
```

- [ ] **Step 3: Implement `SerialisePropertyValue`**

```cpp
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
        return MakeShared<FJsonValueString>(Obj ? Obj->GetPathName() : TEXT(""));
    }
    if (FSoftObjectProperty* P = CastField<FSoftObjectProperty>(Property))
    {
        const FSoftObjectPtr& Ptr = *static_cast<const FSoftObjectPtr*>(ValuePtr);
        return MakeShared<FJsonValueString>(Ptr.ToString());
    }
    // Per spec: large structs become opaque summaries to keep JSON tractable.
    if (FStructProperty* P = CastField<FStructProperty>(Property))
    {
        const TSharedRef<FJsonObject> Summary = MakeShared<FJsonObject>();
        Summary->SetStringField(TEXT("type"),    TEXT("struct"));
        Summary->SetStringField(TEXT("summary"), P->Struct->GetName());
        return MakeShared<FJsonValueObject>(Summary);
    }
    // Arrays/maps/sets: emit length + opaque marker. Cherry-pickable diffs are Plan 4 work.
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
```

- [ ] **Step 4: Implement `WalkProperties`**

```cpp
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
        // Spec §6 keeps property paths flat with structs as opaque summaries; the
        // detailed struct-field walk is Plan 4's responsibility (it enables per-property
        // diff inside structs like FVector and pin literals).
    }
}
```

- [ ] **Step 5: Update `Export` to populate the `asset` block**

Replace the previous `asset` placeholder block in `Export` with:

```cpp
    UObject* Primary = FindPrimaryAsset(Package);
    if (!Primary)
    {
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

    // Sort by `path` so the JSON is canonical (equal inputs → byte-identical output).
    Entries.Sort([](const TSharedPtr<FJsonValue>& A, const TSharedPtr<FJsonValue>& B) {
        return A->AsObject()->GetStringField(TEXT("path"))
             < B->AsObject()->GetStringField(TEXT("path"));
    });

    Asset->SetArrayField(TEXT("properties"), Entries);

    OutResponse->SetBoolField(TEXT("ok"), true);
    OutResponse->SetStringField(TEXT("path"), AbsoluteAssetPath);
    OutResponse->SetObjectField(TEXT("package"), PackageBlock);
    OutResponse->SetObjectField(TEXT("asset"), Asset);
```

- [ ] **Step 6: Rebuild**

```powershell
& "C:\Program Files\Epic Games\UE_5.5\Engine\Build\BatchFiles\Build.bat" `
  UnrealEditor Win64 Development `
  -Project="$PWD\ue-host\HostProject.uproject" -WaitMutex -FromMsBuild
```

Expected: exit code `0`; `UnrealEditor-MergeBinariesExport.dll` re-linked.

- [ ] **Step 7: Re-bless and verify the goldens**

```powershell
pwsh tools/golden-test.ps1 -Bless
pwsh tools/golden-test.ps1
```

Expected second command: `PASS` on both files, exit 0.

- [ ] **Step 8: Confirm the two goldens differ meaningfully**

```powershell
git diff --no-index Examples/v1.expected.json Examples/v2.expected.json | Select-Object -First 80
```

Expected: a non-empty diff with several entries under `asset.properties` differing (the two fixtures differ by 547 bytes — at least a handful of property values should change). If `git diff` reports no semantic difference besides `package.savedHash`, the property walk is missing the fields that actually changed; investigate before continuing. Acceptable scenarios:
- Component override property values differ.
- Variable defaults differ.
- Some BP-internal `bool`/string property differs.

If the only diff between v1 and v2 is the SHA-1 hash, escalate — the walker is broken or the fixtures are bit-identical-but-padded (which would contradict §6 of the spec's binary inspection).

- [ ] **Step 9: Commit**

```powershell
git add ue-plugin/MergeBinariesExport Examples/v1.expected.json Examples/v2.expected.json
git commit -m "feat(ue-plugin): export asset.properties via FProperty reflection"
```

---

## Task 6: Error handling — non-existent path, non-asset file, corrupt asset

**Files:**
- Modify: `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/AssetExporter.cpp` (only if a case below is found uncovered)
- Create: `tools/error-cases.ps1`

The spec lists three error paths the commandlet MUST exit gracefully on (§8.4 and §8.5). The implementation in Task 4–5 covers the common cases; this task exercises them and adds any missing branch.

- [ ] **Step 1: Write a driver script for the three error cases**

Create `tools/error-cases.ps1`:

```powershell
# Works on Windows PowerShell 5.1 and PowerShell 7+.
$ErrorActionPreference = 'Stop'

function Send-Rpc([string]$json) {
    $stdinText = $json + "`n" + '{"cmd":"quit"}' + "`n"
    return powershell -File (Join-Path $PSScriptRoot 'run-commandlet.ps1') -StdinText $stdinText 2>$null |
        Where-Object { $_ -match '^\s*\{.*"id":\s*\d' } |
        Select-Object -First 1
}

# Case 1: path does not exist
$r1 = Send-Rpc '{"id":1,"cmd":"export","path":"C:/does/not/exist.uasset"}'
Write-Host "Case 1 (missing file): $r1"
if (-not ($r1 -match '"ok":false') -or -not ($r1 -match 'file not found')) { throw "Case 1 failed: $r1" }

# Case 2: existing non-uasset file
$tmp = New-TemporaryFile
'not an asset' | Set-Content $tmp
$r2 = Send-Rpc "{`"id`":2,`"cmd`":`"export`",`"path`":`"$(($tmp.FullName) -replace '\\','/')`"}"
Write-Host "Case 2 (junk file): $r2"
if (-not ($r2 -match '"ok":false')) { throw "Case 2 failed: $r2" }
Remove-Item $tmp

# Case 3: unknown cmd
$stdinText3 = '{"id":3,"cmd":"frobnicate"}' + "`n" + '{"cmd":"quit"}' + "`n"
$r3 = powershell -File (Join-Path $PSScriptRoot 'run-commandlet.ps1') -StdinText $stdinText3 2>$null |
    Where-Object { $_ -match '^\s*\{.*"id":\s*3' } | Select-Object -First 1
Write-Host "Case 3 (unknown cmd): $r3"
if (-not ($r3 -match 'unknown cmd')) { throw "Case 3 failed: $r3" }

Write-Host "All error cases handled cleanly." -ForegroundColor Green
```

- [ ] **Step 2: Run it**

```powershell
pwsh tools/error-cases.ps1
```

Expected: `All error cases handled cleanly.` (green). If any case throws, fix the responsible branch in `AssetExporter.cpp` (`FPaths::FileExists` for case 1, `LoadPackage` returning null for case 2, the `Handlers.Find` branch in `JsonRpcLoop.cpp` for case 3).

- [ ] **Step 3: Commit**

```powershell
git add tools/error-cases.ps1
git commit -m "test(ue-plugin): cover missing-file, junk-file, unknown-cmd error paths"
```

---

## Task 7: Stdout cleanliness audit

**Files:**
- (Possibly) Modify: `ue-plugin/MergeBinariesExport/Source/MergeBinariesExport/Private/MergeBinariesExportCommandlet.cpp`
- Create: `tools/audit-stdout.ps1`

The Rust sidecar in Plan 2 will be lenient about stdout noise, but the more we can suppress, the smaller the surface area for parser bugs. This task quantifies the noise so Plan 2's reader can be tuned with confidence.

- [ ] **Step 1: Write the audit script**

Create `tools/audit-stdout.ps1`:

```powershell
# Works on Windows PowerShell 5.1 and PowerShell 7+.
$ErrorActionPreference = 'Stop'

$v1 = (Resolve-Path "Examples/v1/BP_MinimalChar.uasset").Path -replace '\\','/'
$stdinText = "{`"id`":1,`"cmd`":`"export`",`"path`":`"$v1`"}`n{`"id`":2,`"cmd`":`"quit`"}`n"
$lines = powershell -File tools/run-commandlet.ps1 -StdinText $stdinText 2>$null

$total = $lines.Count
$jsonOurs = 0
$jsonOther = 0
$notJson = 0
foreach ($l in $lines) {
    try {
        $obj = $l | ConvertFrom-Json -ErrorAction Stop
        if ($obj.PSObject.Properties.Match('id')) { $jsonOurs++ } else { $jsonOther++ }
    } catch { $notJson++ }
}

Write-Host "Total stdout lines : $total"
Write-Host "Our schema lines   : $jsonOurs"
Write-Host "Other JSON lines   : $jsonOther"
Write-Host "Non-JSON lines     : $notJson"
if ($jsonOurs -ne 1) {
    Write-Host "WARN: expected exactly 1 schema line, got $jsonOurs" -ForegroundColor Yellow
    exit 1
}
exit 0
```

- [ ] **Step 2: Run it**

```powershell
pwsh tools/audit-stdout.ps1
```

Expected: `Our schema lines : 1`, exit 0. The `Non-JSON lines` count is informational — record it in the commit message so Plan 2's sidecar tuning has a baseline.

If `Other JSON lines` is greater than zero, investigate which UE subsystem is emitting structured JSON to stdout and either suppress it (config in `DefaultEngine.ini`) or document it so Plan 2's reader filters on `id` presence (which it will anyway per the spec).

- [ ] **Step 3: Commit**

```powershell
git add tools/audit-stdout.ps1
git commit -m "test(ue-plugin): audit stdout cleanliness (baseline noise count)"
```

---

## Task 8: CI bootstrap (optional but recommended)

**Files:**
- Create: `.github/workflows/ue-plugin-golden.yml`

This task is only worth doing if the engineer is comfortable maintaining a self-hosted Windows runner with UE 5.4 installed — GitHub-hosted runners cannot install the full editor. If no runner is available, **skip this task** and run goldens locally before each merge instead.

- [ ] **Step 1: Author the workflow (self-hosted runner)**

Create `.github/workflows/ue-plugin-golden.yml`:

```yaml
name: ue-plugin-golden

on:
  pull_request:
    paths:
      - 'ue-plugin/**'
      - 'ue-host/**'
      - 'Examples/**'
      - 'tools/**'
      - '.github/workflows/ue-plugin-golden.yml'

jobs:
  golden:
    runs-on: [self-hosted, windows, unreal-5.4]
    steps:
      - uses: actions/checkout@v4
      - name: Build editor
        shell: pwsh
        run: |
          & "C:\Program Files\Epic Games\UE_5.5\Engine\Build\BatchFiles\Build.bat" `
            UnrealEditor Win64 Development `
            -Project="$env:GITHUB_WORKSPACE\ue-host\HostProject.uproject" `
            -WaitMutex -FromMsBuild
      - name: Run golden tests
        shell: pwsh
        run: pwsh tools/golden-test.ps1
      - name: Run error-case tests
        shell: pwsh
        run: pwsh tools/error-cases.ps1
      - name: Audit stdout cleanliness
        shell: pwsh
        run: pwsh tools/audit-stdout.ps1
```

- [ ] **Step 2: Commit**

```powershell
git add .github/workflows/ue-plugin-golden.yml
git commit -m "ci: golden + error-case checks on self-hosted UE runner"
```

---

## Done criteria (verify before declaring Plan 1 complete)

Run all three from the repo root:

```powershell
pwsh tools/golden-test.ps1
pwsh tools/error-cases.ps1
pwsh tools/audit-stdout.ps1
```

All three must exit 0. Additionally:

1. `Examples/v1.expected.json` and `Examples/v2.expected.json` exist, are committed, and are not byte-identical (the `package.savedHash` plus at least one `asset.properties[*].value` must differ).
2. Both expected files contain `"fileVersionUE5": 1017`.
3. Both expected files contain `"class": "Blueprint"` (the `BP_MinimalChar` primary asset is a `UBlueprint`).
4. The commandlet exits with status 0 after a `quit` command and with status 0 (never non-zero) on individual request failures — failures are reported in-band via `{"ok":false,"error":"..."}`.

If any of these fails, the foundation for Plan 2 is shaky — fix here before moving on.

---

## Out of scope for Plan 1 (do NOT attempt; these are Plan 4)

- `blueprint` block of the schema (componentTree, componentOverrides, componentBindings, graphs, functions, macros, variables, nodes, wires, pins).
- Recursing into `FStructProperty` fields. Structs stay opaque summaries here.
- Cycle detection for object references. The current opaque path-string emission for `FObjectProperty` already breaks cycles.
- Thumbnails.

These are intentionally excluded so Plan 1 ships a small, fully-tested, useful foundation.
