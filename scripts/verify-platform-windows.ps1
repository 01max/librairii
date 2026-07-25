$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

$HostOS = (& go env GOOS).Trim()
$HostArch = (& go env GOARCH).Trim()
if ($HostOS -ne "windows" -or $HostArch -ne "amd64") {
    throw "Windows release verification requires windows/amd64, got $HostOS/$HostArch"
}

$Version = (Get-Content VERSION -Raw).Trim()
$WailsVersion = (Get-Content .wails-version -Raw).Trim()
if ($env:WAILS) {
    $WailsCLI = $env:WAILS
} else {
    $WailsCLI = Join-Path ((& go env GOPATH).Trim()) "bin/wails.exe"
}
$ActualWailsVersion = ((& $WailsCLI version) | Select-Object -First 1).Trim()
if ($LASTEXITCODE -ne 0 -or $ActualWailsVersion -ne $WailsVersion) {
    throw "Wails CLI must match $WailsVersion"
}

& $WailsCLI build `
    -clean `
    -trimpath `
    -m `
    -nocolour `
    -platform windows/amd64 `
    -windowsconsole `
    -webview2 error `
    -nsis `
    -installscope user `
    -o Librairii.exe
if ($LASTEXITCODE -ne 0) {
    throw "Wails Windows build failed"
}

$Binary = Join-Path $ProjectRoot "build/bin/Librairii.exe"
$Installer = Join-Path $ProjectRoot "build/bin/Librairii-amd64-installer.exe"
foreach ($Path in @($Binary, $Installer)) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Packaged Windows artifact is missing: $Path"
    }
}

$BinaryBytes = [System.IO.File]::ReadAllBytes($Binary)
if ($BinaryBytes.Length -lt 512) {
    throw "Packaged Windows executable is truncated"
}
$PEOffset = [BitConverter]::ToInt32($BinaryBytes, 0x3c)
if (
    $PEOffset -lt 0 -or
    $PEOffset + 6 -gt $BinaryBytes.Length -or
    [BitConverter]::ToUInt32($BinaryBytes, $PEOffset) -ne 0x00004550 -or
    [BitConverter]::ToUInt16($BinaryBytes, $PEOffset + 4) -ne 0x8664
) {
    throw "Packaged Windows executable is not an amd64 PE image"
}

$VerificationRoot = Join-Path `
    ([System.IO.Path]::GetTempPath()) `
    ("librairii-windows-release-" + [Guid]::NewGuid().ToString("N"))
$DataRoot = Join-Path $VerificationRoot "application-data"
$EventLog = Join-Path $DataRoot "logs/events.jsonl"
New-Item -ItemType Directory -Path $DataRoot | Out-Null

function Invoke-Go {
    & go @args
    if ($LASTEXITCODE -ne 0) {
        throw "go $($args -join ' ') failed"
    }
}

function Get-EventCount {
    param([string]$Event)

    if (-not (Test-Path -LiteralPath $EventLog -PathType Leaf)) {
        return 0
    }
    return @(
        Select-String `
            -LiteralPath $EventLog `
            -SimpleMatch `
            -Pattern "`"event`":`"$Event`""
    ).Count
}

try {
    Invoke-Go run ./cmd/release-smoke -root $DataRoot
    Invoke-Go run ./cmd/foundation-smoke `
        -root $DataRoot `
        -expect-stories 1 `
        -expect-shelves 3

    $env:LIBRAIRII_DATA_ROOT = $DataRoot
    $env:LIBRAIRII_ACCEPTANCE_LOG = "1"
    $env:LIBRAIRII_SMOKE_EXIT = "1"
    $env:LIBRAIRII_SMOKE_HOLD_MS = "1200"

    foreach ($LaunchNumber in 1..2) {
        $StartedBefore = Get-EventCount "runtime_started"
        $StoppedBefore = Get-EventCount "runtime_stopped"
        $LaunchLog = Join-Path $VerificationRoot "launch-$LaunchNumber.log"

        $Output = (& $Binary 2>&1 | Out-String)
        $ExitCode = $LASTEXITCODE
        Set-Content -LiteralPath $LaunchLog -Value $Output -Encoding utf8
        if ($ExitCode -ne 0) {
            throw "Packaged Windows application launch $LaunchNumber failed with $ExitCode"
        }
        if ($Output -notmatch "frontend:rendered") {
            throw "Packaged Windows frontend did not report a completed render"
        }
        if (
            (Get-EventCount "runtime_started") -ne ($StartedBefore + 1) -or
            (Get-EventCount "runtime_stopped") -ne ($StoppedBefore + 1)
        ) {
            throw "Packaged Windows lifecycle did not start and stop cleanly"
        }
        $LastEvent = Get-Content -LiteralPath $EventLog | Select-Object -Last 1
        if ($LastEvent -notmatch '"event":"runtime_stopped","state":"stopped"') {
            throw "Packaged Windows lifecycle did not end in a clean shutdown"
        }
    }

    Invoke-Go run ./cmd/foundation-smoke `
        -root $DataRoot `
        -expect-stories 1 `
        -expect-shelves 3
    Invoke-Go test . -count=1
    Invoke-Go test ./internal/platform `
        -run "^(TestRuntimeDialogsUseNativeMultiFilePicker|TestRuntimeDialogsSupportSingleFileAndDirectory|TestRuntimeDialogsRevealOnlyValidatedDirectory|TestDestinationRevealerUsesPlatformFileManager)$" `
        -count=1
    Invoke-Go test ./cmd/release-smoke `
        -run "^TestReleaseSmokeCoversTheCompleteHeadlessComposition$" `
        -count=1
} finally {
    if (
        (Test-Path -LiteralPath $VerificationRoot) -and
        (Split-Path -Leaf $VerificationRoot) -like "librairii-windows-release-*"
    ) {
        Remove-Item -LiteralPath $VerificationRoot -Recurse -Force
    }
}

$InstallerHash = (Get-FileHash -LiteralPath $Installer -Algorithm SHA256).Hash.ToLowerInvariant()
$ChecksumPath = "$Installer.sha256"
$InstallerName = Split-Path -Leaf $Installer
Set-Content `
    -LiteralPath $ChecksumPath `
    -Value "$InstallerHash  $InstallerName" `
    -Encoding ascii
if (
    (Get-FileHash -LiteralPath $Installer -Algorithm SHA256).Hash.ToLowerInvariant() -ne
    $InstallerHash
) {
    throw "Windows installer checksum verification failed"
}

Write-Host "Windows platform acceptance passed: build, render, SQLite, native adapters, smoke"
Write-Host "Windows installer: $Installer"
