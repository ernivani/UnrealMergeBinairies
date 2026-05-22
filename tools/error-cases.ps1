<#
    Exercises the three error paths the spec (§8.4-8.5) requires the commandlet
    to surface in-band as {"ok":false,"error":"..."} responses (NOT process crashes):
      1. export of a path that does not exist
      2. export of a file that exists but is not a .uasset
      3. dispatch of an unknown cmd

    Usage:
        powershell tools/error-cases.ps1
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$root      = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$launcher  = Join-Path $PSScriptRoot 'run-commandlet.ps1'

function Send-Rpc([string]$json) {
    $stdinText = $json + "`n" + '{"cmd":"quit"}' + "`n"
    $lines = & $launcher -StdinText $stdinText 2>$null
    foreach ($line in $lines) {
        try {
            $obj = $line | ConvertFrom-Json -ErrorAction Stop
        } catch { continue }
        if ($obj -isnot [pscustomobject]) { continue }
        if ($obj.PSObject.Properties.Match('id').Count -eq 0) { continue }
        if ($obj.id -ne 0) { return $obj }   # skip the warmup-ping response
    }
    return $null
}

$failed = $false

# Case 1: path does not exist
$r1 = Send-Rpc '{"id":1,"cmd":"export","path":"C:/does/not/exist.uasset"}'
if (-not $r1)                          { Write-Host "FAIL: case 1 - no response" -ForegroundColor Red; $failed = $true }
elseif ($r1.ok -ne $false)             { Write-Host "FAIL: case 1 - ok should be false, got: $($r1 | ConvertTo-Json -Compress)" -ForegroundColor Red; $failed = $true }
elseif ($r1.error -notlike '*file not found*') { Write-Host "FAIL: case 1 - error mismatch: $($r1.error)" -ForegroundColor Red; $failed = $true }
else                                   { Write-Host "PASS: case 1 (missing file): $($r1.error)" -ForegroundColor Green }

# Case 2: existing non-uasset file
$tmp = New-TemporaryFile
'not an asset' | Set-Content $tmp.FullName
$tmpFwd = ($tmp.FullName -replace '\\','/')
$r2 = Send-Rpc ('{"id":2,"cmd":"export","path":"' + $tmpFwd + '"}')
if (-not $r2)                          { Write-Host "FAIL: case 2 - no response" -ForegroundColor Red; $failed = $true }
elseif ($r2.ok -ne $false)             { Write-Host "FAIL: case 2 - ok should be false, got: $($r2 | ConvertTo-Json -Compress)" -ForegroundColor Red; $failed = $true }
else                                   { Write-Host "PASS: case 2 (junk file): $($r2.error)" -ForegroundColor Green }
Remove-Item $tmp.FullName -ErrorAction SilentlyContinue

# Case 3: unknown cmd
$r3 = Send-Rpc '{"id":3,"cmd":"frobnicate"}'
if (-not $r3)                          { Write-Host "FAIL: case 3 - no response" -ForegroundColor Red; $failed = $true }
elseif ($r3.ok -ne $false)             { Write-Host "FAIL: case 3 - ok should be false, got: $($r3 | ConvertTo-Json -Compress)" -ForegroundColor Red; $failed = $true }
elseif ($r3.error -notlike '*unknown cmd*') { Write-Host "FAIL: case 3 - error mismatch: $($r3.error)" -ForegroundColor Red; $failed = $true }
else                                   { Write-Host "PASS: case 3 (unknown cmd): $($r3.error)" -ForegroundColor Green }

if ($failed) {
    Write-Host "FAIL - see above" -ForegroundColor Red
    exit 1
} else {
    Write-Host "All error cases handled cleanly." -ForegroundColor Green
    exit 0
}
