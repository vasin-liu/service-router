# M2 engineering baseline: mirrors GitHub ci.yml mock profile gates (includes config-snapshot).
# Optional: $env:M2_WITH_DOCKER_PROBE = '1' for compose + doctor --probe-upstream (needs Docker).
$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$config = if ($env:SERVICE_ROUTER_CONFIG) { $env:SERVICE_ROUTER_CONFIG } else { 'config/mock-config.yaml' }

function Wait-LocalPort {
    param([int]$Port, [int]$TimeoutSec = 20)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        try {
            $client = New-Object System.Net.Sockets.TcpClient
            $client.Connect('127.0.0.1', $Port)
            $client.Close()
            return
        }
        catch {
            Start-Sleep -Seconds 1
        }
    }
    throw "port $Port did not become reachable within ${TimeoutSec}s"
}

Write-Host '[m2-baseline] cargo check'
cargo check

Write-Host '[m2-baseline] cargo test'
cargo test -- --nocapture

Write-Host "[m2-baseline] check-config --strict ($config)"
cargo run -- check-config $config --json --strict

Write-Host '[m2-baseline] doctor --json'
cargo run -- doctor --config $config --json

Write-Host '[m2-baseline] route-explain smoke'
cargo run -- route-explain /api/orders/123 GET --config $config --json

Write-Host '[m2-baseline] config-snapshot (stdout)'
cargo run -- config-snapshot --config $config -o -

if ($env:M2_WITH_DOCKER_PROBE -eq '1') {
    Write-Host '[m2-baseline] docker compose up (doctor-probe.compose.yml)'
    docker compose -f .github/compose/doctor-probe.compose.yml up -d
    Write-Host '[m2-baseline] wait for TCP 9000 9001'
    Wait-LocalPort -Port 9000
    Wait-LocalPort -Port 9001
    Write-Host '[m2-baseline] doctor --probe-upstream --json'
    cargo run -- doctor --config $config --probe-upstream --json
    docker compose -f .github/compose/doctor-probe.compose.yml down -v
}

Write-Host '[m2-baseline] OK'
Write-Host '[m2-baseline] tip: for §7 JSON artifacts, set SERVICE_ROUTER_ACCEPTANCE_RUN_GLOBAL=0 and run docs/release-acceptance.ps1 (see docs/m2-release-readiness.md)'