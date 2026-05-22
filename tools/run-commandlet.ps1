# Works on Windows PowerShell 5.1 and PowerShell 7+. No PS7-only syntax used.
[CmdletBinding()]
param(
    [string]$UnrealEditor = "C:\Program Files\Epic Games\UE_5.5\Engine\Binaries\Win64\UnrealEditor.exe",
    [string]$HostProject  = (Join-Path $PSScriptRoot "..\ue-host\HostProject.uproject"),
    [string[]]$ExtraArgs  = @(),
    # When set, this exact text is written as UTF-8 (no BOM) to the child's stdin.
    # Use this whenever you need to send JSON-RPC frames — bypasses PowerShell's
    # default pipe encoding ($OutputEncoding) which can mangle JSON in some shells.
    [string]$StdinText    = $null
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
    # Bytes-direct stdin path: build a ProcessStartInfo, redirect stdin, write UTF-8 bytes,
    # then close stdin so UE sees EOF and the JsonRpcLoop exits cleanly when it runs out of input.
    function Quote-Arg([string]$a) {
        if ($a -match '[\s"]') { '"' + ($a -replace '"', '\"') + '"' } else { $a }
    }
    $argString = ($ueArgs | ForEach-Object { Quote-Arg $_ }) -join ' '

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $UnrealEditor
    $psi.Arguments = $argString
    $psi.RedirectStandardInput = $true
    $psi.UseShellExecute = $false

    $proc = [System.Diagnostics.Process]::Start($psi)
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    $bytes = $utf8NoBom.GetBytes($StdinText)
    $proc.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
    $proc.StandardInput.BaseStream.Flush()
    $proc.StandardInput.Close()
    $proc.WaitForExit()
    exit $proc.ExitCode
} else {
    # Inherit parent's stdin (used when caller redirects via < file or wants no stdin).
    & $UnrealEditor @ueArgs
    exit $LASTEXITCODE
}
