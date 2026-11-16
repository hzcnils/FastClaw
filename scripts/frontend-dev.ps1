# Frontend for `tauri dev`: reuse healthy Vite on 1420, or start a new one.
# Keeps the process alive so Tauri does not treat beforeDevCommand as exited.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $root

function Test-ViteAlive {
  try {
    $r = Invoke-WebRequest -Uri 'http://127.0.0.1:1420/' -UseBasicParsing -TimeoutSec 3
    return ($r.StatusCode -eq 200)
  } catch {
    return $false
  }
}

if (Test-ViteAlive) {
  Write-Host '[frontend-dev] Reusing Vite on http://127.0.0.1:1420' -ForegroundColor Green
  while ($true) { Start-Sleep -Seconds 3600 }
}

Write-Host '[frontend-dev] Starting Vite...' -ForegroundColor Cyan
npm run dev
