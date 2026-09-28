$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$server = Join-Path $projectRoot "runtime\llama\llama-server.exe"
if (-not (Test-Path -LiteralPath $server)) {
  & (Join-Path $PSScriptRoot "setup-llm.ps1")
} else {
  Write-Host "llama.cpp runtime is ready."
}
