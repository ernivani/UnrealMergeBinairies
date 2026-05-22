<#
    Drives a single export against Examples/v1/BP_MinimalChar.uasset and reports
    how much noise UE puts on stdout alongside our JSON frames. Baseline data for
    Plan 2's Rust sidecar reader (which will need to be tolerant of any leftover
    boot/init/log lines that escape -AbsLog).

    Two views:
      - Raw stdout (UE invoked directly): counts ALL lines, categorises into
        "our schema" (parses as JSON with an `id` field), "other JSON", "non-JSON".
      - Launcher-extracted (run-commandlet.ps1 -StdinText): should be all JSON
        frames, post-extractor.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$root      = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$launcher  = Join-Path $PSScriptRoot 'run-commandlet.ps1'

$v1 = (Resolve-Path (Join-Path $root 'Examples\v1\BP_MinimalChar.uasset')).Path
$rpc = '{"id":0,"cmd":"_warmup"}' + "`n" +
       '{"id":1,"cmd":"export","path":"' + ($v1 -replace '\\','/') + '"}' + "`n" +
       '{"id":2,"cmd":"quit"}' + "`n"

# ---- View A: raw stdout via direct UnrealEditor.exe invocation ---------------
$ueExe       = 'C:\Program Files\Epic Games\UE_5.6\Engine\Binaries\Win64\UnrealEditor.exe'
$hostProject = Join-Path $root 'ue-host\HostProject.uproject'
$logFile     = Join-Path ([System.IO.Path]::GetTempPath()) ("MBE-audit-" + [Guid]::NewGuid().ToString() + ".log")

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $ueExe
$psi.Arguments = '"' + $hostProject + '" -run=MergeBinariesExport -stdio -nullrhi -unattended -NoCrashReports -AbsLog=' + $logFile
$psi.RedirectStandardInput  = $true
$psi.RedirectStandardOutput = $true
$psi.UseShellExecute = $false
$proc = [System.Diagnostics.Process]::Start($psi)
$utf8 = New-Object System.Text.UTF8Encoding($false)
$bytes = $utf8.GetBytes($rpc)
$proc.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
$proc.StandardInput.BaseStream.Flush()
$proc.StandardInput.Close()
$rawText = $proc.StandardOutput.ReadToEnd()
$proc.WaitForExit()

$rawLines = $rawText -split "`r?`n"
$totalRaw = ($rawLines | Where-Object { $_.Length -gt 0 }).Count

$schemaCount = 0
$otherJsonCount = 0
$nonJsonCount   = 0
foreach ($line in $rawLines) {
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    try {
        $obj = $line | ConvertFrom-Json -ErrorAction Stop
        if ($obj -is [pscustomobject] -and $obj.PSObject.Properties.Match('id').Count -gt 0) {
            $schemaCount++
        } else {
            $otherJsonCount++
        }
    } catch {
        $nonJsonCount++
    }
}

Write-Host "=== Raw stdout (UE invoked directly with -AbsLog) ==="
Write-Host "Total non-empty lines : $totalRaw"
Write-Host "Our schema lines      : $schemaCount  (expected: 3 - warmup + export + quit)"
Write-Host "Other JSON lines      : $otherJsonCount"
Write-Host "Non-JSON noise lines  : $nonJsonCount"

# ---- View B: post-extractor via the launcher --------------------------------
$launched = & $launcher -StdinText $rpc 2>$null
$launchedJson = 0
$launchedOther = 0
foreach ($line in $launched) {
    try {
        $obj = $line | ConvertFrom-Json -ErrorAction Stop
        if ($obj -is [pscustomobject] -and $obj.PSObject.Properties.Match('id').Count -gt 0) {
            $launchedJson++
        } else {
            $launchedOther++
        }
    } catch { $launchedOther++ }
}

Write-Host ""
Write-Host "=== Post-extractor (run-commandlet.ps1 brace-counter) ==="
Write-Host "Schema lines : $launchedJson"
Write-Host "Other        : $launchedOther"

# Cleanup
if (Test-Path $logFile) { Remove-Item $logFile -ErrorAction SilentlyContinue }

if ($schemaCount -lt 2) {
    Write-Host ""
    Write-Host "WARN: expected at least 2 schema lines on raw stdout, got $schemaCount" -ForegroundColor Yellow
    exit 1
}
exit 0
