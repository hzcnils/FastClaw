$ErrorActionPreference = 'Stop'

$forbiddenTracked = @(
  'data/FastKGService/',
  'data/File2Md/',
  'data/shimmy/',
  'src-tauri/binaries/',
  'src-tauri/intent-model/',
  'public/WebResource/',
  'login-download-packages/',
  'crates/mnemosyne/',
  'src/components/auth/',
  'src/integrations/account/',
  'src/views/profile/',
  'fastclaw.sql',
  'node_modules.zip'
)

$tracked = @(git -c core.quotepath=false ls-files)
$violations = foreach ($prefix in $forbiddenTracked) {
  $tracked | Where-Object { $_ -eq $prefix -or $_.StartsWith($prefix) }
}

$largeFiles = foreach ($file in $tracked) {
  if (Test-Path -LiteralPath $file -PathType Leaf) {
    $item = Get-Item -LiteralPath $file
    if ($item.Length -gt 90MB) { $file }
  }
}

$secretPattern = '(^|[^A-Za-z])sk-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9]{20,}|-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----'
$secretHits = @(git grep -n -I -E $secretPattern -- '*.ts' '*.tsx' '*.js' '*.vue' '*.rs' '*.sql' '*.json' '*.toml' '*.html' 2>$null)
$secretHits = $secretHits | Where-Object {
  $_ -notmatch '__tests__|\.test\.|\.spec\.|sk-xxxx|sk-complete-|sk-full-|sk-management-|sk-another-'
}

if ($violations -or $largeFiles -or $secretHits) {
  if ($violations) { Write-Error "Forbidden runtime files are tracked:`n$($violations -join "`n")" }
  if ($largeFiles) { Write-Error "Tracked files exceed 90 MB:`n$($largeFiles -join "`n")" }
  if ($secretHits) { Write-Error "Potential credentials detected:`n$($secretHits -join "`n")" }
  exit 1
}

Write-Host 'Public release check passed.'
