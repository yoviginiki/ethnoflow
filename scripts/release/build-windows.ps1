# EthnoFlow Studio — Windows release build (run from repo root in PowerShell)
# Prereqs: Rust (MSVC), Node 22, WebView2 runtime. NSIS/WiX are fetched by tauri-cli.
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..\..")

Write-Host "== Rust tests ==" -ForegroundColor Cyan
cargo test --workspace
if ($LASTEXITCODE -ne 0) { throw "cargo test failed" }

Write-Host "== Frontend ==" -ForegroundColor Cyan
Set-Location apps\desktop
npm ci
npm run typecheck
if ($LASTEXITCODE -ne 0) { throw "typecheck failed" }
npm test
if ($LASTEXITCODE -ne 0) { throw "vitest failed" }

Write-Host "== Tauri build (release + installers) ==" -ForegroundColor Cyan
npm run tauri build
if ($LASTEXITCODE -ne 0) { throw "tauri build failed" }

Set-Location ..\..
$bundle = "target\release\bundle"
Write-Host "== Artifacts ==" -ForegroundColor Cyan
Get-ChildItem -Recurse $bundle -Include *.exe,*.msi | ForEach-Object {
  "{0}  {1:N1} MB" -f $_.FullName, ($_.Length / 1MB)
}
if (-not (Get-ChildItem -Recurse $bundle -Include *.exe,*.msi)) { throw "no installer produced" }
