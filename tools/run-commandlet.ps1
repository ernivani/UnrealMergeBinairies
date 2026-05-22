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
    [string]$UnrealEditor = "C:\Program Files\Epic Games\UE_5.5\Engine\Binaries\Win64\UnrealEditor.exe",
    [string]$HostProject  = (Join-Path $PSScriptRoot "..\ue-host\HostProject.uproject"),
    [string[]]$ExtraArgs  = @(),
    [string]$StdinText    = $null,
    # Skip the warmup-ping prepend (set this if you're sending control frames the loop
    # must see as the very first line, e.g. an end-to-end timing test).
    [switch]$NoWarmup
)

if (-not (Test-Path -LiteralPath $UnrealEditor)) {
    Write-Error "UnrealEditor.exe not found at: $UnrealEditor"
    exit 127
}
if (-not (Test-Path -LiteralPath $HostProject)) {
    Write-Error "HostProject.uproject not found at: $HostProject"
    exit 127
}

$ueArgs = @(
    $HostProject,
    "-run=MergeBinariesExport",
    "-stdio",
    "-nullrhi",
    "-unattended",
    "-NoCrashReports"
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

    # Emit stdout lines to PowerShell output stream so callers can pipe.
    foreach ($line in $stdoutLines) { Write-Output $line }
    # Emit stderr to host stderr (PowerShell's error stream) for visibility, not pipeline.
    foreach ($line in $stderrLines) { [Console]::Error.WriteLine($line) }

    exit $proc.ExitCode
} else {
    # Inherit parent's stdio. Used for interactive / smoke launches.
    & $UnrealEditor @ueArgs
    exit $LASTEXITCODE
}
