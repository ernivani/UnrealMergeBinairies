<#
    Drives MergeBinariesExport against every fixture under Examples/v*/*.uasset,
    captures the JSON response for each, normalises volatile fields, and diffs
    against the matching Examples/<n>.expected.json.

    Usage:
        powershell tools/golden-test.ps1            # verify
        powershell tools/golden-test.ps1 -Bless     # overwrite expected files with current output
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

# Sacrificial first line: UE's stdio attach consumes the first stdin line before
# our JsonRpcLoop starts reading. Send a harmless `ping` so the real exports survive.
$rpcLines = @('{"id":0,"cmd":"ping"}')
$rpcLines += $requests | ForEach-Object {
    [pscustomobject]@{ id = $_.id; cmd = $_.cmd; path = $_.path } | ConvertTo-Json -Compress
}
$rpcLines += '{"cmd":"quit"}'

$stdinText = ($rpcLines -join "`n") + "`n"

# Launch UnrealEditor directly so we can fully capture stdout (where our JSON
# responses go) without it leaking to the console. run-commandlet.ps1 inherits
# stdout from the parent and is therefore unsuitable for programmatic capture.
$UnrealEditor = 'C:\Program Files\Epic Games\UE_5.5\Engine\Binaries\Win64\UnrealEditor.exe'
$HostProject  = Join-Path $Root 'ue-host\HostProject.uproject'
$ueArgs = @("`"$HostProject`"", '-run=MergeBinariesExport', '-stdio', '-nullrhi', '-unattended', '-NoCrashReports')

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName  = $UnrealEditor
$psi.Arguments = ($ueArgs -join ' ')
$psi.RedirectStandardInput  = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError  = $true
$psi.UseShellExecute        = $false
$psi.StandardOutputEncoding = New-Object System.Text.UTF8Encoding($false)
$psi.StandardErrorEncoding  = New-Object System.Text.UTF8Encoding($false)

$proc = [System.Diagnostics.Process]::Start($psi)

$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$bytes = $utf8NoBom.GetBytes($stdinText)
$proc.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
$proc.StandardInput.BaseStream.Flush()
$proc.StandardInput.Close()

# UE writes all log noise + our JSON responses to stdout (stderr stays empty).
# Drain stderr after stdout to keep the deadlock surface minimal.
$rawText  = $proc.StandardOutput.ReadToEnd()
$null     = $proc.StandardError.ReadToEnd()
$proc.WaitForExit()

# UE log output and our JSON responses share stdout, so log timestamps can be
# glued onto JSON lines. Walk the text and extract balanced top-level `{...}`
# objects, then filter for ones that carry an "id" field.
$responses = @{}
$len = $rawText.Length
for ($i = 0; $i -lt $len; $i++) {
    if ($rawText[$i] -ne '{') { continue }
    $depth = 0
    $inStr = $false
    $esc   = $false
    $start = $i
    for ($j = $i; $j -lt $len; $j++) {
        $c = $rawText[$j]
        if ($inStr) {
            if ($esc)              { $esc = $false }
            elseif ($c -eq '\')    { $esc = $true }
            elseif ($c -eq '"')    { $inStr = $false }
        } else {
            if     ($c -eq '"')    { $inStr = $true }
            elseif ($c -eq '{')    { $depth++ }
            elseif ($c -eq '}')    {
                $depth--
                if ($depth -eq 0) {
                    $candidate = $rawText.Substring($start, $j - $start + 1)
                    try {
                        $obj = $candidate | ConvertFrom-Json -ErrorAction Stop
                        if ($obj -is [pscustomobject] -and $obj.PSObject.Properties.Match('id').Count -gt 0) {
                            $responses[[int]$obj.id] = $obj
                        }
                    } catch { }
                    $i = $j
                    break
                }
            }
        }
    }
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
    # Strip the absolute on-disk path so the golden file is portable across machines.
    if ($Obj.PSObject.Properties.Match('path').Count -gt 0) { $Obj.path = '<ABSOLUTE_PATH_STRIPPED>' }
    # Engine patch version drifts; pin to major.minor only.
    if ($Obj.package -and $Obj.package.engineVersion) {
        $Obj.package.engineVersion = ($Obj.package.engineVersion -replace '^(\d+\.\d+).*','$1.x')
    }
    # Strip the per-request id â€” only the (path, tag) identifies the fixture.
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

