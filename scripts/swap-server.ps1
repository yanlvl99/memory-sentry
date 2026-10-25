# Swap the running Memory Sentry server for a freshly built one without restarting Spotify.
# The old instance restores its limits and exits, the new build takes its place, and the
# plugin reconnects by itself on its next poll. Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\swap-server.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\swap-server.ps1 -SkipBuild

param([switch]$SkipBuild)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
$Target = Join-Path $Root 'native\rust\target\x86_64-pc-windows-msvc\release\memory-sentry-native.exe'

if (-not $SkipBuild) {
    cargo build --release --manifest-path (Join-Path $Root 'native\rust\Cargo.toml') --target x86_64-pc-windows-msvc --locked
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE." }
}

if (-not (Test-Path -LiteralPath $Target)) { throw "Built server not found at $Target." }

& $Target --swap $Target
exit $LASTEXITCODE
