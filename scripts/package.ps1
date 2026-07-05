<#
.SYNOPSIS
    Assemble a portable, shareable Xenon (whispr) build in dist/Xenon.

.DESCRIPTION
    Packages the already-built release binaries + the AI models/runtime they
    need into a self-contained folder that runs outside the dev repo:

        dist/Xenon/whispr-app.exe
        dist/Xenon/*.dll                  (sherpa + onnx runtime)
        dist/Xenon/models/                (ASR + LLM models)
        dist/Xenon/llama/llama-server.exe (+ its dlls)

    This matches the PACKAGED layout resolved by whispr-core's
    default_models_root() / llm::find_server_exe() at runtime.

    Assumes `cargo build --release` has already been run.

.NOTES
    Idempotent: dist/Xenon is deleted and recreated fresh on every run.
#>

[CmdletBinding()]
param(
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

# Repo root = parent of this script's directory (scripts/..).
$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $RepoRoot

$Release   = Join-Path $RepoRoot 'target\release'
$BenchDir  = Join-Path $RepoRoot 'bench\models'
$LlamaDir  = Join-Path $RepoRoot 'tools\llama'
$DistRoot  = Join-Path $RepoRoot 'dist'
$Dist      = Join-Path $DistRoot 'Xenon'

$Sources = @(
    @{ Label = 'whispr-app.exe';                  Path = Join-Path $Release 'whispr-app.exe' },
    @{ Label = 'release DLLs (target\release\*.dll)'; Path = (Join-Path $Release '*.dll') },
    @{ Label = 'parakeet model dir';               Path = Join-Path $BenchDir 'sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8' },
    @{ Label = 'moonshine model dir';              Path = Join-Path $BenchDir 'sherpa-onnx-moonshine-base-en-int8' },
    @{ Label = 'qwen gguf';                        Path = Join-Path $BenchDir 'qwen2.5-1.5b-instruct-q4_k_m.gguf' },
    @{ Label = 'llama-server.exe';                 Path = Join-Path $LlamaDir 'llama-server.exe' },
    @{ Label = 'llama dlls (tools\llama\*.dll)';   Path = (Join-Path $LlamaDir '*.dll') }
)

Write-Host "== Sanity-checking source paths ==" -ForegroundColor Cyan
$missing = @()
foreach ($s in $Sources) {
    $ok = Test-Path $s.Path
    $status = if ($ok) { 'OK  ' } else { 'MISS' }
    $color = if ($ok) { 'Green' } else { 'Red' }
    Write-Host ("  [{0}] {1}  ({2})" -f $status, $s.Label, $s.Path) -ForegroundColor $color
    if (-not $ok) { $missing += $s.Label }
}
if ($missing.Count -gt 0) {
    throw "Missing source path(s): $($missing -join ', '). Build the workspace (cargo build --release) and/or extract bench/models first."
}

if ($DryRun) {
    Write-Host "`n-DryRun specified: not copying anything. Sources look OK." -ForegroundColor Yellow
    return
}

Write-Host "`n== Recreating $Dist ==" -ForegroundColor Cyan
if (Test-Path $Dist) {
    Remove-Item -Recurse -Force $Dist
}
New-Item -ItemType Directory -Force -Path $Dist | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $Dist 'models') | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $Dist 'llama') | Out-Null

Write-Host "== Copying whispr-app.exe ==" -ForegroundColor Cyan
Copy-Item -Path (Join-Path $Release 'whispr-app.exe') -Destination $Dist -Force

Write-Host "== Copying runtime DLLs (sherpa/onnx) ==" -ForegroundColor Cyan
Copy-Item -Path (Join-Path $Release '*.dll') -Destination $Dist -Force

Write-Host "== Copying models (~1.5 GB, this may take a while) ==" -ForegroundColor Cyan
$ModelsDest = Join-Path $Dist 'models'
Copy-Item -Path (Join-Path $BenchDir 'sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8') -Destination $ModelsDest -Recurse -Force
Copy-Item -Path (Join-Path $BenchDir 'sherpa-onnx-moonshine-base-en-int8') -Destination $ModelsDest -Recurse -Force
Copy-Item -Path (Join-Path $BenchDir 'qwen2.5-1.5b-instruct-q4_k_m.gguf') -Destination $ModelsDest -Force

Write-Host "== Copying llama-server + its DLLs ==" -ForegroundColor Cyan
$LlamaDest = Join-Path $Dist 'llama'
Copy-Item -Path (Join-Path $LlamaDir 'llama-server.exe') -Destination $LlamaDest -Force
Copy-Item -Path (Join-Path $LlamaDir '*.dll') -Destination $LlamaDest -Force

$sizeBytes = (Get-ChildItem -Recurse -Force $Dist | Measure-Object -Property Length -Sum).Sum
$sizeGB = [math]::Round($sizeBytes / 1GB, 2)

Write-Host "`n== Done ==" -ForegroundColor Green
Write-Host ("Total size: {0} GB" -f $sizeGB)
Write-Host "Folder ready at $Dist — run whispr-app.exe" -ForegroundColor Green
