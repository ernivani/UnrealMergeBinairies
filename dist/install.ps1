# Installs the MergeBinariesExport editor plugin into the UE 5.6 engine.
# Run from the extracted tool folder:  ./install.ps1   [-EnginePath "D:\Epic\UE_5.6"]
param(
    [string]$EnginePath = "C:\Program Files\Epic Games\UE_5.6"
)

$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path

if ($here -match '\s') {
    Write-Warning "This folder path contains spaces ($here). Git's merge driver will fail to launch the exe."
    Write-Warning "Move this folder to a no-spaces path (e.g. C:\UnrealMergeTool) and re-run."
}

$src = Join-Path $here "EnginePlugin\MergeBinariesExport"
if (-not (Test-Path $src)) { throw "Plugin source not found at $src" }

$enginePlugins = Join-Path $EnginePath "Engine\Plugins"
if (-not (Test-Path $enginePlugins)) {
    throw "UE engine not found at $EnginePath (no Engine\Plugins). Pass -EnginePath '<your UE_5.6>'."
}

$dst = Join-Path $enginePlugins "MergeBinariesExport"
Write-Host "Installing plugin -> $dst"
if (Test-Path $dst) { Remove-Item -Recurse -Force $dst }
Copy-Item -Recurse -Force $src $dst

Write-Host ""
Write-Host "Done. Plugin installed as an engine plugin."
Write-Host "Next, in EACH game repo run:"
Write-Host "    $here\unreal-merge.exe install"
Write-Host "Then 'git merge' will launch the tool on .uasset/.umap conflicts."
