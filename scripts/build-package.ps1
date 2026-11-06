[CmdletBinding()]
param([string]$Version = "")

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

if (-not $Version) {
    $cargo = Get-Content -LiteralPath (Join-Path $Root "server\Cargo.toml") -Raw
    if ($cargo -notmatch '(?m)^version\s*=\s*"([^"]+)"') { throw "Could not read server version from Cargo.toml." }
    $Version = $Matches[1]
}
if ($Version -notmatch '^[0-9A-Za-z][0-9A-Za-z.+-]*$') { throw "Invalid release version: $Version" }

function Write-Sha256([string]$Path) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try { $stream = [System.IO.File]::OpenRead($Path); try { $h = ([System.BitConverter]::ToString($sha.ComputeHash($stream)) -replace '-', '').ToLowerInvariant() } finally { $stream.Dispose() } } finally { $sha.Dispose() }
    Set-Content -LiteralPath ($Path + ".sha256") -Value ($h + "  " + [IO.Path]::GetFileName($Path)) -Encoding ASCII
    Write-Host ("SHA-256 " + [IO.Path]::GetFileName($Path) + ": " + $h)
}

$CargoManifest = Join-Path $Root "server\Cargo.toml"
$Dist = Join-Path $Root "dist"
$WixProjDir = Join-Path $Root "installer\wix"

$WixCommand = Get-Command "wix" -ErrorAction SilentlyContinue
if (-not $WixCommand) { $WixCommand = Get-Command "wix.exe" -ErrorAction SilentlyContinue }
if (-not $WixCommand) { throw "WiX toolset (wix) was not found. Install the .NET SDK and run 'dotnet tool install --global wix', then add the UI and Util extensions." }
if (-not (Test-Path -LiteralPath (Join-Path $WixProjDir "Product.wxs") -PathType Leaf)) { throw "The WiX project installer\wix\Product.wxs is missing." }

# Build a native server and MSI for every supported architecture. Only the host architecture's
# binary can be executed for the version/self-test smoke checks; the others are cross-compiled and
# packaged but not run on this machine.
$HostArch = switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture) {
    "Arm64" { "arm64" }
    default { "x64" }
}
$Architectures = @(
    @{ Name = "x64";   Target = "x86_64-pc-windows-msvc";  WixArch = "x64" },
    @{ Name = "arm64"; Target = "aarch64-pc-windows-msvc"; WixArch = "arm64" }
)

# Prepare one shared staging folder. The architecture-independent payload (Custom App and docs) is
# copied once; only MemorySentry.exe is replaced per architecture before each MSI build.
$Stage = Join-Path $Dist "stage"
New-Item -ItemType Directory -Path $Dist -Force | Out-Null
if (Test-Path -LiteralPath $Stage) { Remove-Item -LiteralPath $Stage -Recurse -Force }
$OldPortable = Join-Path $Dist "portable"
if (Test-Path -LiteralPath $OldPortable) { Remove-Item -LiteralPath $OldPortable -Recurse -Force }
New-Item -ItemType Directory -Path (Join-Path $Stage "app") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $Stage "docs") -Force | Out-Null
Copy-Item -Path (Join-Path $Root "app\*") -Destination (Join-Path $Stage "app") -Recurse -Force
foreach ($doc in @("README.md", "LICENSE", "PRIVACY.md", "SECURITY.md")) {
    Copy-Item -LiteralPath (Join-Path $Root $doc) -Destination $Stage -Force
}
foreach ($doc in @("INSTALLATION.md", "TROUBLESHOOTING.md")) {
    Copy-Item -LiteralPath (Join-Path $Root ("docs\" + $doc)) -Destination (Join-Path $Stage "docs") -Force
}

foreach ($arch in $Architectures) {
    $Name = $arch.Name
    Write-Host ("Building native " + $Name + " server...")
    & cargo build --release --manifest-path $CargoManifest --target $arch.Target --locked
    if ($LASTEXITCODE -ne 0) { throw "Rust release build for " + $Name + " failed with exit code $LASTEXITCODE." }

    $Binary = Join-Path $Root ("server\target\" + $arch.Target + "\release\memory-sentry-server.exe")
    if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) {
        throw "Compiled " + $Name + " server not found at $Binary."
    }

    # Only the host architecture's binary can run here, so the smoke checks are skipped otherwise.
    if ($Name -eq $HostArch) {
        $VersionOutput = & $Binary "--version" 2>&1
        if ($LASTEXITCODE -ne 0 -or ($VersionOutput -join " ") -notmatch [regex]::Escape($Version)) {
            throw "Compiled " + $Name + " binary version does not match package version $Version."
        }
        & $Binary "--self-test"
        if ($LASTEXITCODE -ne 0) { throw "The " + $Name + " binary self-test failed." }
    }

    Copy-Item -LiteralPath $Binary -Destination (Join-Path $Stage "MemorySentry.exe") -Force

    $Msi = Join-Path $Dist ("MemorySentryForSpicetify-" + $Version + "-" + $Name + ".msi")
    Push-Location $WixProjDir
    try {
        & $WixCommand.Source build -ext WixToolset.UI.wixext -ext WixToolset.Util.wixext -arch $arch.WixArch -o $Msi "Product.wxs"
        if ($LASTEXITCODE -ne 0) { throw "WiX MSI build for " + $Name + " failed with exit code $LASTEXITCODE." }
    } finally { Pop-Location }
    if (-not (Test-Path -LiteralPath $Msi -PathType Leaf)) { throw "WiX completed without producing $Msi." }
    Write-Sha256 $Msi
    Write-Host ("Official installer (" + $Name + "): " + $Msi)
}
