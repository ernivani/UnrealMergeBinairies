# Works on Windows PowerShell 5.1 and PowerShell 7+. No PS7-only syntax used.
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
