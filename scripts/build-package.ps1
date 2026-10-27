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

$InnoCandidates = @(
    (Join-Path ${env:ProgramFiles(x86)} "Inno Setup 6\ISCC.exe"),
    (Join-Path $env:ProgramFiles "Inno Setup 6\ISCC.exe"),
    (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe")
)
$ISCC = $InnoCandidates | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
if (-not $ISCC) {
    $command = Get-Command "ISCC.exe" -ErrorAction SilentlyContinue
    if ($command) { $ISCC = $command.Source }
}
if (-not $ISCC) { throw "Inno Setup 6 compiler (ISCC.exe) was not found. Install Inno Setup 6 before packaging." }

$Iss = Join-Path $Root "installer\inno\MemorySentry.iss"
& $ISCC ("/DAppVersion=" + $Version) ("/DStageDir=" + $Stage) $Iss
if ($LASTEXITCODE -ne 0) { throw "Inno Setup compilation failed with exit code $LASTEXITCODE." }
$SetupExe = Join-Path $Dist ("MemorySentrySetup-" + $Version + ".exe")
if (-not (Test-Path -LiteralPath $SetupExe -PathType Leaf)) { throw "Inno Setup completed without producing $SetupExe." }
$sha = [System.Security.Cryptography.SHA256]::Create()
try { $stream = [System.IO.File]::OpenRead($SetupExe); try { $Hash = ([System.BitConverter]::ToString($sha.ComputeHash($stream)) -replace '-', '').ToLowerInvariant() } finally { $stream.Dispose() } } finally { $sha.Dispose() }
$HashPath = $SetupExe + ".sha256"
Set-Content -LiteralPath $HashPath -Value ($Hash + "  " + [IO.Path]::GetFileName($SetupExe)) -Encoding ASCII
Write-Host ("SHA-256 " + [IO.Path]::GetFileName($SetupExe) + ": " + $Hash)
Write-Host "Official installer: $SetupExe"
Write-Host "Checksum: $HashPath"
