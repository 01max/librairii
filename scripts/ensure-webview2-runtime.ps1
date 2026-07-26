$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RuntimeClientID = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
$RuntimeRegistryPaths = @(
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$RuntimeClientID",
    "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$RuntimeClientID"
)
$BootstrapperURL = "https://go.microsoft.com/fwlink/p/?LinkId=2124703"

function Get-WebView2Runtime {
    foreach ($RegistryPath in $RuntimeRegistryPaths) {
        $Properties = Get-ItemProperty `
            -LiteralPath $RegistryPath `
            -ErrorAction SilentlyContinue
        if ($null -eq $Properties) {
            continue
        }
        $VersionProperty = $Properties.PSObject.Properties["pv"]
        if ($null -eq $VersionProperty) {
            continue
        }
        $Version = $VersionProperty.Value
        if ($Version -and $Version -ne "0.0.0.0") {
            return [PSCustomObject]@{
                RegistryPath = $RegistryPath
                Version = $Version
            }
        }
    }
    return $null
}

$Runtime = Get-WebView2Runtime
if ($null -ne $Runtime) {
    Write-Host (
        "WebView2 Runtime $($Runtime.Version) detected at " +
        $Runtime.RegistryPath
    )
    exit 0
}

Write-Host "WebView2 Runtime was not detected; installing Evergreen Runtime"
$BootstrapperDirectory = Join-Path `
    ([System.IO.Path]::GetTempPath()) `
    ("librairii-webview2-" + [Guid]::NewGuid().ToString("N"))
$Bootstrapper = Join-Path `
    $BootstrapperDirectory `
    "MicrosoftEdgeWebview2Setup.exe"

try {
    New-Item -ItemType Directory -Path $BootstrapperDirectory | Out-Null
    Invoke-WebRequest -Uri $BootstrapperURL -OutFile $Bootstrapper

    $Signature = Get-AuthenticodeSignature -LiteralPath $Bootstrapper
    if (
        $Signature.Status -ne [System.Management.Automation.SignatureStatus]::Valid -or
        $Signature.SignerCertificate.Subject -notlike "*Microsoft Corporation*"
    ) {
        throw "Downloaded WebView2 bootstrapper does not have a valid Microsoft signature"
    }

    $InstallProcess = Start-Process `
        -FilePath $Bootstrapper `
        -ArgumentList @("/silent", "/install") `
        -Wait `
        -PassThru

    foreach ($Attempt in 1..30) {
        $Runtime = Get-WebView2Runtime
        if ($null -ne $Runtime) {
            break
        }
        Start-Sleep -Seconds 2
    }
    if ($null -eq $Runtime) {
        throw (
            "WebView2 Runtime was not registered after installation; " +
            "bootstrapper exit code $($InstallProcess.ExitCode)"
        )
    }
    Write-Host (
        "WebView2 Runtime $($Runtime.Version) installed and detected at " +
        $Runtime.RegistryPath
    )
} finally {
    if (
        (Test-Path -LiteralPath $BootstrapperDirectory) -and
        (Split-Path -Leaf $BootstrapperDirectory) -like "librairii-webview2-*"
    ) {
        Remove-Item -LiteralPath $BootstrapperDirectory -Recurse -Force
    }
}
