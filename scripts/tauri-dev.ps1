# FastClaw 开发启动：编译产物在项目 src-tauri\target；临时目录用系统默认（可用 FASTCLAW_TMP 覆盖）
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

# 仅当显式设置 FASTCLAW_TMP 时才改 TMP/TEMP，否则走系统默认路径
if ($env:FASTCLAW_TMP) {
  $tmp = $env:FASTCLAW_TMP
  New-Item -ItemType Directory -Force -Path $tmp | Out-Null
  $env:TMP = $tmp
  $env:TEMP = $tmp
  Write-Host "[tauri-dev] TMP/TEMP => $tmp" -ForegroundColor DarkGray
} else {
  Write-Host "[tauri-dev] TMP/TEMP => system default ($env:TEMP)" -ForegroundColor DarkGray
}

$env:CARGO_TARGET_DIR = Join-Path $root 'src-tauri\target'
$env:RUST_MIN_STACK = '16777216'
$env:NODE_OPTIONS = '--max-old-space-size=8192'

# Port 1420: only kill unresponsive Vite; do not kill a healthy instance
try {
  $conns = @(Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue)
  if ($conns.Count -gt 0) {
    $alive = $false
    try {
      $probe = Invoke-WebRequest -Uri 'http://127.0.0.1:1420/' -UseBasicParsing -TimeoutSec 2
      if ($probe.StatusCode -eq 200) { $alive = $true }
    } catch {
      $alive = $false
    }
    if (-not $alive) {
      Write-Host '[tauri-dev] Port 1420 stuck; clearing zombie Vite...' -ForegroundColor Yellow
      foreach ($c in $conns) {
        if ($c.OwningProcess) {
          Stop-Process -Id $c.OwningProcess -Force -ErrorAction SilentlyContinue
        }
      }
      Start-Sleep -Seconds 1
    } else {
      Write-Host '[tauri-dev] Port 1420 already has a live Vite. Avoid running multiple tauri:dev.' -ForegroundColor Yellow
    }
  }
} catch { }

$viteCache = Join-Path $root 'node_modules\.vite'
if ($env:CLEAR_VITE_CACHE -eq '1' -and (Test-Path $viteCache)) {
  Remove-Item -Recurse -Force $viteCache -ErrorAction SilentlyContinue
}

Set-Location $root
npm run tauri -- dev --release --features dev-db,devtools @args
