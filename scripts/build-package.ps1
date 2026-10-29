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

$CargoManifest = Join-Path $Root "server\Cargo.toml"
& cargo build --release --manifest-path $CargoManifest --target x86_64-pc-windows-msvc --locked
if ($LASTEXITCODE -ne 0) { throw "Rust release build failed with exit code $LASTEXITCODE." }

$Binary = Join-Path $Root "server\target\x86_64-pc-windows-msvc\release\memory-sentry-server.exe"
if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) {
    throw "Compiled Windows server not found at $Binary. Run cargo build --release for x86_64-pc-windows-msvc first."
}
$VersionOutput = & $Binary "--version" 2>&1
if ($LASTEXITCODE -ne 0 -or ($VersionOutput -join " ") -notmatch [regex]::Escape($Version)) {
    throw "Compiled binary version does not match requested package version $Version."
}
& $Binary "--self-test"
if ($LASTEXITCODE -ne 0) { throw "The binary self-test failed." }

$Dist = Join-Path $Root "dist"
$Stage = Join-Path $Dist "stage"
New-Item -ItemType Directory -Path $Dist -Force | Out-Null
if (Test-Path -LiteralPath $Stage) { Remove-Item -LiteralPath $Stage -Recurse -Force }
$OldPortable = Join-Path $Dist "portable"
if (Test-Path -LiteralPath $OldPortable) { Remove-Item -LiteralPath $OldPortable -Recurse -Force }
New-Item -ItemType Directory -Path (Join-Path $Stage "app") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $Stage "docs") -Force | Out-Null
Copy-Item -LiteralPath $Binary -Destination (Join-Path $Stage "MemorySentry.exe") -Force
Copy-Item -Path (Join-Path $Root "app\*") -Destination (Join-Path $Stage "app") -Recurse -Force
foreach ($name in @("README.md", "LICENSE", "PRIVACY.md", "SECURITY.md")) {
    Copy-Item -LiteralPath (Join-Path $Root $name) -Destination $Stage -Force
}
foreach ($name in @("INSTALLATION.md", "TROUBLESHOOTING.md")) {
    Copy-Item -LiteralPath (Join-Path $Root ("docs\" + $name)) -Destination (Join-Path $Stage "docs") -Force
}

function Write-Sha256([string]$Path) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try { $stream = [System.IO.File]::OpenRead($Path); try { $h = ([System.BitConverter]::ToString($sha.ComputeHash($stream)) -replace '-', '').ToLowerInvariant() } finally { $stream.Dispose() } } finally { $sha.Dispose() }
    Set-Content -LiteralPath ($Path + ".sha256") -Value ($h + "  " + [IO.Path]::GetFileName($Path)) -Encoding ASCII
    Write-Host ("SHA-256 " + [IO.Path]::GetFileName($Path) + ": " + $h)
}

# --- MSI (WiX v5) build ---
$WixCommand = Get-Command "wix" -ErrorAction SilentlyContinue
if (-not $WixCommand) { $WixCommand = Get-Command "wix.exe" -ErrorAction SilentlyContinue }
if (-not $WixCommand) { throw "WiX toolset (wix) was not found. Install the .NET SDK and run 'dotnet tool install --global wix', then add the UI and Util extensions." }
$WixProjDir = Join-Path $Root "installer\wix"
if (-not (Test-Path -LiteralPath (Join-Path $WixProjDir "Product.wxs") -PathType Leaf)) { throw "The WiX project installer\wix\Product.wxs is missing." }
$Msi = Join-Path $Dist ("MemorySentryForSpicetify-" + $Version + ".msi")
Push-Location $WixProjDir
try {
    & $WixCommand.Source build -ext WixToolset.UI.wixext -ext WixToolset.Util.wixext -arch x64 -o $Msi "Product.wxs"
    if ($LASTEXITCODE -ne 0) { throw "WiX MSI build failed with exit code $LASTEXITCODE." }
} finally { Pop-Location }
if (-not (Test-Path -LiteralPath $Msi -PathType Leaf)) { throw "WiX completed without producing $Msi." }
Write-Sha256 $Msi
Write-Host "Official installer: $Msi"

