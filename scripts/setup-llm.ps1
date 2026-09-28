param(
  [string]$ModelRepo = "Qwen/Qwen3-4B-GGUF",
  [string]$ModelFile = "Qwen3-4B-Q4_K_M.gguf",
  [string]$MirrorEndpoint = "https://hf-mirror.com"
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$runtimeDirectory = Join-Path $projectRoot "runtime\llama"
$server = Join-Path $runtimeDirectory "llama-server.exe"

if (-not (Test-Path -LiteralPath $server)) {
  Write-Host "Installing the official llama.cpp Vulkan runtime..."
  winget install --id ggml.llamacpp --exact --accept-package-agreements --accept-source-agreements --silent
  $package = Get-ChildItem -LiteralPath (Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages") -Directory |
    Where-Object Name -Like "ggml.llamacpp_*" |
    Select-Object -First 1
  if (-not $package) { throw "llama.cpp was installed but its package directory was not found." }
  New-Item -ItemType Directory -Force -Path $runtimeDirectory | Out-Null
  Get-ChildItem -LiteralPath $package.FullName -File | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination $runtimeDirectory -Force
  }
}

$python = Join-Path $projectRoot ".venv-asr\Scripts\python.exe"
if (-not (Test-Path -LiteralPath $python)) {
  & (Join-Path $PSScriptRoot "setup-asr.ps1")
}
& $python -m pip install "huggingface_hub>=0.27,<2" --index-url "https://pypi.tuna.tsinghua.edu.cn/simple"

$modelDirectory = Join-Path ([Environment]::GetFolderPath("ApplicationData")) "com.voicedesk.desktop\models\llm"
New-Item -ItemType Directory -Force -Path $modelDirectory | Out-Null
$target = Join-Path $modelDirectory $ModelFile
if (-not (Test-Path -LiteralPath $target) -or (Get-Item -LiteralPath $target).Length -lt 1GB) {
  $download = @"
from huggingface_hub import hf_hub_download
hf_hub_download(repo_id='$ModelRepo', filename='$ModelFile', local_dir=r'$modelDirectory')
"@
  $env:HF_ENDPOINT = $MirrorEndpoint
  $download | & $python -
  if ($LASTEXITCODE -ne 0) {
    Write-Warning "The mirror was unavailable; falling back to the official Hugging Face endpoint."
    Remove-Item Env:HF_ENDPOINT -ErrorAction SilentlyContinue
    $download | & $python -
    if ($LASTEXITCODE -ne 0) { throw "Model download failed from both mirror and official sources." }
  }
}

& $server --version
Write-Host "VoiceDesk LLM runtime is ready: $server"
Write-Host "VoiceDesk LLM model is ready: $target"
