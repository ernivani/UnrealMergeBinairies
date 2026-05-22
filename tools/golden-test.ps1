<#
    Drives MergeBinariesExport against every fixture under Examples/v*/*.uasset,
    captures the JSON response for each, normalises volatile fields, and diffs
    against the matching Examples/<n>.expected.json.

    Usage:
        powershell tools/golden-test.ps1            # verify
        powershell tools/golden-test.ps1 -Bless     # overwrite expected files
#>
[CmdletBinding()]
param(
    [switch]$Bless
)

$ErrorActionPreference = 'Stop'
$Root        = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$ExamplesDir = Join-Path $Root 'Examples'
$Versions    = Get-ChildItem $ExamplesDir -Directory | Where-Object Name -Match '^v\d+$' | Sort-Object Name

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

# Drive the commandlet through the canonical launcher. run-commandlet.ps1 handles:
#   - UTF-8 stdin (bypasses PowerShell pipe encoding)
#   - the warmup-ping that absorbs UE's first-stdin-line eating
#   - routing UE logs to a temp file via -AbsLog
#   - extracting balanced top-level JSON objects from stdout (one per Write-Output)
$launcher = Join-Path $PSScriptRoot 'run-commandlet.ps1'
$outLines = & $launcher -StdinText $stdinText 2>$null

# Each line is already a clean JSON object courtesy of the launcher's brace-counter.
# Parse them, key by `id`, ignore the warmup response (id=0) and the quit ack (no id).
$responses = @{}
foreach ($line in $outLines) {
    try {
        $obj = $line | ConvertFrom-Json -ErrorAction Stop
    } catch { continue }
    if ($obj -isnot [pscustomobject]) { continue }
    if ($obj.PSObject.Properties.Match('id').Count -eq 0) { continue }
    $responses[[int]$obj.id] = $obj
}

# Recursively sort object keys so JSON comparison is order-independent.
function Sort-JsonKeys($Node) {
    if ($null -eq $Node) { return $null }
    if ($Node -is [System.Collections.IDictionary]) {
        $ordered = [ordered]@{}
        foreach ($k in ($Node.Keys | Sort-Object)) { $ordered[$k] = Sort-JsonKeys $Node[$k] }
        return $ordered
    }
    if ($Node -is [pscustomobject]) {
        $ordered = [ordered]@{}
        foreach ($p in ($Node.PSObject.Properties | Sort-Object Name)) { $ordered[$p.Name] = Sort-JsonKeys $p.Value }
        return $ordered
    }
    if ($Node -is [System.Collections.IEnumerable] -and -not ($Node -is [string])) {
        return @($Node | ForEach-Object { Sort-JsonKeys $_ })
    }
    return $Node
}

function Format-CanonicalJson($Obj) {
    return ((Sort-JsonKeys $Obj) | ConvertTo-Json -Depth 64)
}

function Normalise([pscustomobject]$Obj) {
    # Strip the absolute on-disk path so the golden is portable across machines.
    if ($Obj.PSObject.Properties.Match('path').Count -gt 0) { $Obj.path = '<ABSOLUTE_PATH_STRIPPED>' }
    # Engine patch version drifts; pin to major.minor only.
    if ($Obj.package -and $Obj.package.engineVersion) {
        $Obj.package.engineVersion = ($Obj.package.engineVersion -replace '^(\d+\.\d+).*','$1.x')
    }
    # Strip the per-request id - only (path, tag) identifies the fixture.
    if ($Obj.PSObject.Properties.Match('id').Count -gt 0) { $Obj.PSObject.Properties.Remove('id') }
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
    $actualJson = Format-CanonicalJson $normalised
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

    $expectedRaw = Get-Content $expectedFile -Raw | ConvertFrom-Json
    $expectedJson = Format-CanonicalJson $expectedRaw
    if ($actualJson -ne $expectedJson) {
        Write-Host "FAIL: diff for $expectedFile" -ForegroundColor Red
        Compare-Object ($expectedJson -split "`n") ($actualJson -split "`n") | Format-Table -AutoSize
        $failed = $true
    } else {
        Write-Host "PASS: $expectedFile" -ForegroundColor Green
    }
}

if ($failed) { exit 1 } else { exit 0 }
