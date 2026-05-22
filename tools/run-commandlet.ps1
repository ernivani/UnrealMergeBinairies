# Works on Windows PowerShell 5.1 and PowerShell 7+. No PS7-only syntax used.
#
# Two modes:
#   - Default: spawns UnrealEditor.exe and inherits the parent's stdin/stdout/stderr.
#     Use this for interactive smoke tests where you want UE's logs visible.
#   - -StdinText "<utf-8 text>": spawns UE with stdin/stdout redirected via
#     System.Diagnostics.Process. Writes the given text to UE's stdin as UTF-8 (no BOM),
#     captures stdout, and emits each stdout line as a PowerShell output object so the
#     caller can pipe (`... | Where-Object { ... }`). This bypasses PowerShell's pipe
#     encoding which can mangle JSON-RPC frames in some shells.
[CmdletBinding()]
param(
    [string]$UnrealEditor = "C:\Program Files\Epic Games\UE_5.6\Engine\Binaries\Win64\UnrealEditor.exe",
    [string]$HostProject  = "",
    [string[]]$ExtraArgs  = @(),
    [string]$StdinText    = $null,
    # Skip the warmup-ping prepend (set this if you're sending control frames the loop
    # must see as the very first line, e.g. an end-to-end timing test).
    [switch]$NoWarmup
)

# Resolve $HostProject default here (not in the param block) because $PSScriptRoot
# is unreliably populated when parameter defaults are evaluated in Windows PowerShell 5.1
# under certain invocation patterns.
if ([string]::IsNullOrEmpty($HostProject)) {
    $scriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
    $HostProject = Join-Path $scriptDir "..\ue-host\HostProject.uproject"
}

if (-not (Test-Path -LiteralPath $UnrealEditor)) {
    Write-Error "UnrealEditor.exe not found at: $UnrealEditor"
    exit 127
}
if (-not (Test-Path -LiteralPath $HostProject)) {
    Write-Error "HostProject.uproject not found at: $HostProject"
    exit 127
}

# -AbsLog routes ALL UE logging (LogLinker warnings, LogAssetRegistry, etc.) into a
# file instead of stdout/stderr. Without this, asynchronous UE log lines splice into
# our JSON-RPC stdout frames mid-line and break consumers' JSON parsers.
$logFile = Join-Path ([System.IO.Path]::GetTempPath()) ("MergeBinariesExport-" + [Guid]::NewGuid().ToString() + ".log")

$ueArgs = @(
    $HostProject,
    "-run=MergeBinariesExport",
    "-stdio",
    "-nullrhi",
    "-unattended",
    "-NoCrashReports",
    "-AbsLog=$logFile"
) + $ExtraArgs

if ($PSBoundParameters.ContainsKey('StdinText')) {
    # Warmup-ping: empirically UE's stdio init on -run= dispatch swallows or corrupts
    # the first stdin line. We prepend a sacrificial frame with id=0 so real client
    # frames (id>=1) survive. Downstream filters that key on id>=1 ignore the warmup
    # response automatically. Disable with -NoWarmup if a caller needs raw control.
    $effectiveStdin = if ($NoWarmup) {
        $StdinText
    } else {
        '{"id":0,"cmd":"_warmup"}' + "`n" + $StdinText
    }

    function Quote-Arg([string]$a) {
        if ($a -match '[\s"]') { '"' + ($a -replace '"', '\"') + '"' } else { $a }
    }
    $argString = ($ueArgs | ForEach-Object { Quote-Arg $_ }) -join ' '

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $UnrealEditor
    $psi.Arguments = $argString
    $psi.RedirectStandardInput  = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError  = $true
    $psi.UseShellExecute = $false

    $proc = New-Object System.Diagnostics.Process
    $proc.StartInfo = $psi

    # Collect stdout/stderr asynchronously to avoid deadlock if either pipe fills.
    $stdoutLines = New-Object System.Collections.Generic.List[string]
    $stderrLines = New-Object System.Collections.Generic.List[string]
    $stdoutHandler = {
        if ($EventArgs.Data -ne $null) { $Event.MessageData.Add($EventArgs.Data) }
    }
    Register-ObjectEvent -InputObject $proc -EventName OutputDataReceived `
        -Action $stdoutHandler -MessageData $stdoutLines | Out-Null
    Register-ObjectEvent -InputObject $proc -EventName ErrorDataReceived `
        -Action $stdoutHandler -MessageData $stderrLines | Out-Null

    $null = $proc.Start()
    $proc.BeginOutputReadLine()
    $proc.BeginErrorReadLine()

    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    $bytes = $utf8NoBom.GetBytes($effectiveStdin)
    $proc.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
    $proc.StandardInput.BaseStream.Flush()
    $proc.StandardInput.Close()

    $proc.WaitForExit()

    # Drain any final async output. Cleanup event subscribers.
    Get-EventSubscriber | Where-Object { $_.SourceObject -eq $proc } | Unregister-Event
    Remove-Event -SourceIdentifier * -ErrorAction SilentlyContinue

    # UE's logging system (LogLinker, LogAssetRegistry, etc.) occasionally writes
    # warning lines to stdout without a leading newline, causing them to glue onto
    # the tail of our JSON-RPC frames mid-line. To shield consumers from this,
    # walk the entire captured stdout as one string and extract every balanced
    # top-level `{...}` block. One Write-Output per extracted block.
    $combined = $stdoutLines -join "`n"
    $depth = 0; $start = -1; $inStr = $false; $esc = $false
    for ($i = 0; $i -lt $combined.Length; $i++) {
        $c = $combined[$i]
        if ($esc) { $esc = $false; continue }
        if ($c -eq '\') { $esc = $true; continue }
        if ($c -eq '"') { $inStr = -not $inStr; continue }
        if ($inStr) { continue }
        if ($c -eq '{') { if ($depth -eq 0) { $start = $i }; $depth++ }
        elseif ($c -eq '}') {
            $depth--
            if ($depth -eq 0 -and $start -ge 0) {
                Write-Output $combined.Substring($start, $i - $start + 1)
                $start = -1
            }
        }
    }
    # Emit stderr to host stderr for visibility, not pipeline.
    foreach ($line in $stderrLines) { [Console]::Error.WriteLine($line) }

    exit $proc.ExitCode
} else {
    # Inherit parent's stdio. Used for interactive / smoke launches.
    & $UnrealEditor @ueArgs
    exit $LASTEXITCODE
}
