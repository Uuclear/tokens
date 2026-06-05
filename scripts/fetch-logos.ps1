# Re-download bundled platform logos into src/serve/assets/logos/
# Run from repo root: .\scripts\fetch-logos.ps1

$ErrorActionPreference = "Stop"
$dir = Join-Path $PSScriptRoot "..\src\serve\assets\logos"
New-Item -ItemType Directory -Force -Path $dir | Out-Null

$primary = @{
  claude_code = "https://claude.ai/images/claude_app_icon.png"
  cursor      = "https://www.cursor.com/apple-touch-icon.png"
  opencode    = "https://opencode.ai/favicon.ico"
  hermes      = "https://www.nousresearch.com/favicon.ico"
  chatbox     = "https://chatboxai.app/favicon.ico"
  postman     = "https://www.postman.com/favicon.ico"
}

$google = @{
  codex          = "openai.com"
  qwen_code      = "qwen.ai"
  openclaw       = "openclaw.ai"
  cherry_studio  = "cherry-ai.com"
  dify           = "dify.ai"
  qoder          = "qoder.com"
  cline          = "cline.bot"
  kilo_cli       = "kilocode.ai"
}

foreach ($k in $primary.Keys) {
  $url = $primary[$k]
  $ext = if ($url -match '\.(png|ico|svg)') { $Matches[0].TrimStart('.') } else { "bin" }
  $out = Join-Path $dir "$k.$ext"
  curl.exe -fsSL -o $out $url
  Write-Host "OK $k"
}

foreach ($k in $google.Keys) {
  $out = Join-Path $dir "$k.png"
  curl.exe -fsSL -o $out "https://www.google.com/s2/favicons?domain=$($google[$k])&sz=128"
  Write-Host "OK $k (png)"
}

Copy-Item (Join-Path $dir "kilo_cli.png") (Join-Path $dir "kilo_ide.png") -Force -ErrorAction SilentlyContinue
Write-Host "Generating theme logos..."
Push-Location (Join-Path $PSScriptRoot "..")
cargo run --bin generate_theme_logos --features logo-gen
Pop-Location
Write-Host "Done. Rebuild: cargo build"
