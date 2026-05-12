param(
    [string]$ConfigPath = $(if ($env:SERVICE_ROUTER_CONFIG) { $env:SERVICE_ROUTER_CONFIG } else { "config/mock-config.yaml" }),
    [string]$SmokePath = $(if ($env:SERVICE_ROUTER_SMOKE_PATH) { $env:SERVICE_ROUTER_SMOKE_PATH } else { "/api/orders/123" }),
    [string]$SmokeMethod = $(if ($env:SERVICE_ROUTER_SMOKE_METHOD) { $env:SERVICE_ROUTER_SMOKE_METHOD } else { "GET" }),
    [string]$ArtifactDir = $(if ($env:SERVICE_ROUTER_ACCEPTANCE_OUT) { $env:SERVICE_ROUTER_ACCEPTANCE_OUT } else { "artifacts/release-acceptance" }),
    [string]$RunGlobal = $(if ($env:SERVICE_ROUTER_ACCEPTANCE_RUN_GLOBAL) { $env:SERVICE_ROUTER_ACCEPTANCE_RUN_GLOBAL } else { "1" }),
    [string]$AllowProbeFail = $(if ($env:SERVICE_ROUTER_ACCEPTANCE_ALLOW_PROBE_FAIL) { $env:SERVICE_ROUTER_ACCEPTANCE_ALLOW_PROBE_FAIL } else { "0" })
)

$ErrorActionPreference = "Stop"

New-Item -ItemType Directory -Path $ArtifactDir -Force | Out-Null

function Invoke-CargoCapture {
    param(
        [string[]]$CargoArgs,
        [string]$OutFile,
        [bool]$AllowFailure = $false
    )

    $output = & cargo @CargoArgs
    $exitCode = $LASTEXITCODE
    $output | Tee-Object -FilePath (Join-Path $ArtifactDir $OutFile)

    if ($exitCode -ne 0 -and -not $AllowFailure) {
        throw "cargo $($CargoArgs -join ' ') failed with exit code $exitCode"
    }
}

Write-Host "[release-acceptance] config: $ConfigPath"
Write-Host "[release-acceptance] smoke: $SmokeMethod $SmokePath"
Write-Host "[release-acceptance] output: $ArtifactDir"

if ($RunGlobal -eq "1") {
    Write-Host "[release-acceptance] global gates: text encoding + cargo check + cargo test"
    python scripts/check-text-encoding.py
    cargo check
    cargo test -- --nocapture
}

Write-Host "[release-acceptance] check-config --strict"
Invoke-CargoCapture -CargoArgs @("run","--","check-config","--config",$ConfigPath,"--json","--strict") -OutFile "check-config.json"

Write-Host "[release-acceptance] doctor"
Invoke-CargoCapture -CargoArgs @("run","--","doctor","--config",$ConfigPath,"--json") -OutFile "doctor.json"

Write-Host "[release-acceptance] doctor --probe-upstream"
Invoke-CargoCapture -CargoArgs @("run","--","doctor","--config",$ConfigPath,"--probe-upstream","--json") -OutFile "doctor-probe.json" -AllowFailure ($AllowProbeFail -eq "1")

Write-Host "[release-acceptance] route-explain smoke"
Invoke-CargoCapture -CargoArgs @("run","--","route-explain",$SmokePath,$SmokeMethod,"--config",$ConfigPath,"--json") -OutFile "route-explain-smoke.json"

Write-Host "[release-acceptance] config-snapshot (redacted)"
$snapshotPath = Join-Path $ArtifactDir "config-snapshot.json"
& cargo @("run","--","config-snapshot","--config",$ConfigPath,"-o",$snapshotPath)
if ($LASTEXITCODE -ne 0) {
    throw "config-snapshot failed with exit code $LASTEXITCODE"
}

Write-Host "[release-acceptance] section-9 summary (markdown)"
$summaryPath = Join-Path $ArtifactDir "section-9-summary.generated.md"
$summary = & python scripts/summarize-section9-release-acceptance.py --artifacts-dir $ArtifactDir
if ($LASTEXITCODE -ne 0) {
    throw "section-9 summary generation failed with exit code $LASTEXITCODE"
}
$summary | Out-File -FilePath $summaryPath -Encoding utf8

Write-Host "[release-acceptance] done"
