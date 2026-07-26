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

$NSISDirectory = Join-Path ${env:ProgramFiles(x86)} "NSIS"
$Makensis = Join-Path $NSISDirectory "makensis.exe"
if (-not (Test-Path -LiteralPath $Makensis -PathType Leaf)) {
    throw "NSIS compiler is missing: $Makensis"
}
$env:Path = "$NSISDirectory;$env:Path"
if (-not (Get-Command makensis.exe -CommandType Application -ErrorAction SilentlyContinue)) {
    throw "NSIS compiler is not available on PATH"
}

& npm --prefix frontend run build
if ($LASTEXITCODE -ne 0) {
    throw "Frontend build failed"
}

& $WailsCLI build `
    -clean `
    -trimpath `
    -m `
    -nocolour `
    -platform windows/amd64 `
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
    $PEOffset + 94 -gt $BinaryBytes.Length -or
    [BitConverter]::ToUInt32($BinaryBytes, $PEOffset) -ne 0x00004550 -or
    [BitConverter]::ToUInt16($BinaryBytes, $PEOffset + 4) -ne 0x8664
) {
    throw "Packaged Windows executable is not an amd64 PE image"
}
$OptionalHeaderOffset = $PEOffset + 24
if (
    [BitConverter]::ToUInt16($BinaryBytes, $OptionalHeaderOffset) -ne 0x020b -or
    [BitConverter]::ToUInt16($BinaryBytes, $OptionalHeaderOffset + 68) -ne 0x0002
) {
    throw "Packaged Windows executable is not a GUI-subsystem PE image"
}

$VerificationRoot = Join-Path `
    ([System.IO.Path]::GetTempPath()) `
    ("librairii-windows-release-" + [Guid]::NewGuid().ToString("N"))
$DataRoot = Join-Path $VerificationRoot "application-data"
$HeadlessRoot = Join-Path $VerificationRoot "headless-data"
$ExportDestination = Join-Path $VerificationRoot "export"
$InstallRoot = Join-Path $VerificationRoot "installed"
$InstalledBinary = Join-Path $InstallRoot "Librairii.exe"
$Uninstaller = Join-Path $InstallRoot "uninstall.exe"
$RetentionMarker = Join-Path $DataRoot "uninstall-retention.txt"
$AcceptanceCheckpoints = Join-Path $VerificationRoot "checkpoints.log"
$AcceptanceSource = Join-Path `
    $ProjectRoot `
    "internal/inspection/testfixture/testdata/generic.7z"
$EventLog = Join-Path $DataRoot "logs/events.jsonl"
$DatabasePath = Join-Path $DataRoot "db/librairii.sqlite3"
$DesktopShortcut = Join-Path `
    ([Environment]::GetFolderPath("Desktop")) `
    "Librairii.lnk"
$StartMenuShortcut = Join-Path `
    ([Environment]::GetFolderPath("Programs")) `
    "Librairii.lnk"
$UninstallRegistry = `
    "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LibrairiiLibrairii"
New-Item `
    -ItemType Directory `
    -Path $DataRoot, $HeadlessRoot, $ExportDestination |
    Out-Null

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

function Invoke-PackagedApplication {
    param(
        [string]$Path,
        [int]$TimeoutMilliseconds = 30000
    )

    $Process = Start-Process -FilePath $Path -PassThru
    if (-not $Process.WaitForExit($TimeoutMilliseconds)) {
        Stop-Process -Id $Process.Id -Force
        throw "Packaged Windows application timed out"
    }
    if ($Process.ExitCode -ne 0) {
        throw "Packaged Windows application exited with $($Process.ExitCode)"
    }
}

$InstallationAttempted = $false
try {
    foreach ($ExistingInstallationState in @(
        $DesktopShortcut,
        $StartMenuShortcut,
        $UninstallRegistry
    )) {
        if (Test-Path -LiteralPath $ExistingInstallationState) {
            throw (
                "Refusing to overwrite an existing Librairii installation: " +
                $ExistingInstallationState
            )
        }
    }

    $InstallationAttempted = $true
    $InstallProcess = Start-Process `
        -FilePath $Installer `
        -ArgumentList @("/S", "/D=$InstallRoot") `
        -Wait `
        -PassThru
    if ($InstallProcess.ExitCode -ne 0) {
        throw "Windows installer exited with $($InstallProcess.ExitCode)"
    }
    foreach ($InstalledPath in @(
        $InstalledBinary,
        $Uninstaller,
        $DesktopShortcut,
        $StartMenuShortcut,
        $UninstallRegistry
    )) {
        if (-not (Test-Path -LiteralPath $InstalledPath)) {
            throw "Windows installer did not create $InstalledPath"
        }
    }

    $InstalledHash = (
        Get-FileHash -LiteralPath $InstalledBinary -Algorithm SHA256
    ).Hash
    $LooseHash = (Get-FileHash -LiteralPath $Binary -Algorithm SHA256).Hash
    if ($InstalledHash -ne $LooseHash) {
        throw "Installed Windows executable differs from the qualified build"
    }

    $ShortcutShell = New-Object -ComObject WScript.Shell
    foreach ($ShortcutPath in @($DesktopShortcut, $StartMenuShortcut)) {
        $ShortcutTarget = $ShortcutShell.CreateShortcut($ShortcutPath).TargetPath
        if (
            [System.IO.Path]::GetFullPath($ShortcutTarget) -ne
            [System.IO.Path]::GetFullPath($InstalledBinary)
        ) {
            throw "Windows shortcut does not target the installed executable"
        }
    }

    $Registration = Get-ItemProperty -LiteralPath $UninstallRegistry
    if (
        [System.IO.Path]::GetFullPath($Registration.DisplayIcon) -ne
        [System.IO.Path]::GetFullPath($InstalledBinary) -or
        $Registration.QuietUninstallString -notlike "*$Uninstaller*"
    ) {
        throw "Windows per-user uninstall registration is incorrect"
    }

    $env:LIBRAIRII_DATA_ROOT = $DataRoot
    $env:LIBRAIRII_PACKAGED_ACCEPTANCE = "1"
    $env:LIBRAIRII_ACCEPTANCE_SOURCE = $AcceptanceSource
    $env:LIBRAIRII_ACCEPTANCE_DESTINATION = $ExportDestination
    $env:LIBRAIRII_ACCEPTANCE_CHECKPOINTS = $AcceptanceCheckpoints
    Invoke-PackagedApplication $InstalledBinary

    $ExpectedCheckpoints = @(
        "scenario_started",
        "native_import_dialog_selected",
        "import_queued",
        "import_succeeded",
        "collection_loaded",
        "native_destination_dialog_selected",
        "export_prepared",
        "export_queued",
        "export_succeeded",
        "native_reveal_succeeded",
        "reveal_succeeded",
        "complete"
    )
    $ActualCheckpoints = @(Get-Content -LiteralPath $AcceptanceCheckpoints)
    if (
        $ActualCheckpoints.Count -ne $ExpectedCheckpoints.Count -or
        (Compare-Object `
            -ReferenceObject $ExpectedCheckpoints `
            -DifferenceObject $ActualCheckpoints `
            -SyncWindow 0)
    ) {
        throw "Packaged Windows bindings did not complete the acceptance scenario"
    }
    foreach ($State in @(
        "import:queued",
        "import:running",
        "import:succeeded",
        "export:queued",
        "export:running",
        "export:succeeded"
    )) {
        if (-not (
            Select-String `
                -LiteralPath $EventLog `
                -SimpleMatch `
                -Quiet `
                -Pattern "`"state`":`"$State`""
        )) {
            throw "Packaged Windows progress log is missing $State"
        }
    }
    if (@(
        Get-ChildItem -LiteralPath $ExportDestination -File -Recurse
    ).Count -ne 1) {
        throw "Packaged Windows export did not publish exactly one archive"
    }

    Invoke-Go run ./cmd/foundation-smoke `
        -root $DataRoot `
        -expect-stories 1 `
        -expect-shelves 0

    Remove-Item Env:LIBRAIRII_PACKAGED_ACCEPTANCE
    Remove-Item Env:LIBRAIRII_ACCEPTANCE_SOURCE
    Remove-Item Env:LIBRAIRII_ACCEPTANCE_DESTINATION
    Remove-Item Env:LIBRAIRII_ACCEPTANCE_CHECKPOINTS

    Invoke-Go run ./cmd/release-smoke -root $HeadlessRoot
    Invoke-Go run ./cmd/foundation-smoke `
        -root $HeadlessRoot `
        -expect-stories 1 `
        -expect-shelves 3

    $env:LIBRAIRII_SMOKE_EXIT = "1"
    $env:LIBRAIRII_SMOKE_HOLD_MS = "1200"

    foreach ($LaunchNumber in 1..2) {
        $StartedBefore = Get-EventCount "runtime_started"
        $StoppedBefore = Get-EventCount "runtime_stopped"
        Invoke-PackagedApplication $InstalledBinary
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
        -expect-shelves 0
    Invoke-Go test . -count=1
    Invoke-Go test ./internal/platform `
        -run "^(TestProductionRuntimeDialogsUseHostNativeAdapters|TestRuntimeDialogsUseNativeMultiFilePicker|TestRuntimeDialogsSupportSingleFileAndDirectory|TestRuntimeDialogsRevealOnlyValidatedDirectory|TestDestinationRevealerUsesPlatformFileManager)$" `
        -count=1
    Invoke-Go test ./cmd/release-smoke `
        -run "^TestReleaseSmokeCoversTheCompleteHeadlessComposition$" `
        -count=1

    Set-Content `
        -LiteralPath $RetentionMarker `
        -Value "preserve user-owned library data" `
        -Encoding ascii
    $UninstallProcess = Start-Process `
        -FilePath $Uninstaller `
        -ArgumentList "/S" `
        -Wait `
        -PassThru
    if ($UninstallProcess.ExitCode -ne 0) {
        throw "Windows uninstaller exited with $($UninstallProcess.ExitCode)"
    }
    foreach ($RemovedInstallationState in @(
        $InstallRoot,
        $DesktopShortcut,
        $StartMenuShortcut,
        $UninstallRegistry
    )) {
        if (Test-Path -LiteralPath $RemovedInstallationState) {
            throw "Windows uninstaller retained $RemovedInstallationState"
        }
    }
    foreach ($RetainedUserData in @($DatabasePath, $RetentionMarker)) {
        if (-not (Test-Path -LiteralPath $RetainedUserData -PathType Leaf)) {
            throw "Windows uninstaller removed user data: $RetainedUserData"
        }
    }
} finally {
    foreach ($AcceptanceVariable in @(
        "LIBRAIRII_PACKAGED_ACCEPTANCE",
        "LIBRAIRII_ACCEPTANCE_SOURCE",
        "LIBRAIRII_ACCEPTANCE_DESTINATION",
        "LIBRAIRII_ACCEPTANCE_CHECKPOINTS",
        "LIBRAIRII_SMOKE_EXIT",
        "LIBRAIRII_SMOKE_HOLD_MS"
    )) {
        Remove-Item "Env:$AcceptanceVariable" -ErrorAction SilentlyContinue
    }
    if ($InstallationAttempted -and (Test-Path -LiteralPath $Uninstaller)) {
        Start-Process `
            -FilePath $Uninstaller `
            -ArgumentList "/S" `
            -Wait |
            Out-Null
    }
    if ($InstallationAttempted) {
        Remove-Item `
            -LiteralPath $DesktopShortcut, $StartMenuShortcut `
            -Force `
            -ErrorAction SilentlyContinue
        Remove-Item `
            -LiteralPath $UninstallRegistry `
            -Recurse `
            -Force `
            -ErrorAction SilentlyContinue
    }
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

Write-Host `
    "Windows platform acceptance passed: install, render, SQLite, host-native dialogs, uninstall"
Write-Host "Windows installer: $Installer"
