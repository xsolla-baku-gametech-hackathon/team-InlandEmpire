# Starts server and bridge, then the game if the Tauri CLI is installed.
# Usage: scripts\demo.ps1          fake taps, mock provider
#        scripts\demo.ps1 COM3     real pad, provider from .env
param([string]$Port = "")
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

if ($Port) {
    $bridgeArgs = @("--port", $Port)
} else {
    $bridgeArgs = @("--fake")
    if (-not $env:TAPPAD_PROVIDER) { $env:TAPPAD_PROVIDER = "mock" }
}

cargo build -q -p tappad-server -p tappad-bridge
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$server = Start-Process -PassThru -NoNewWindow ".\target\debug\tappad-server.exe"
$bridge = Start-Process -PassThru -NoNewWindow ".\target\debug\tappad-bridge.exe" -ArgumentList $bridgeArgs

try {
    cargo tauri --version *> $null
    if ($LASTEXITCODE -eq 0) {
        Push-Location crates\tappad-game
        try { cargo tauri dev } finally { Pop-Location }
    } else {
        Write-Host "no tauri CLI, server and bridge are up, start the game by hand. Ctrl-C stops them."
        Wait-Process -Id $server.Id
    }
} finally {
    foreach ($p in @($server, $bridge)) {
        if ($p -and -not $p.HasExited) { Stop-Process -Id $p.Id -Force }
    }
}
