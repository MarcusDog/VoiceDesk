param(
  [string]$Python = "py -3.11",
  [string]$IndexUrl = "https://pypi.tuna.tsinghua.edu.cn/simple"
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$venv = Join-Path $projectRoot ".venv-asr"

if (-not (Test-Path $venv)) {
  Invoke-Expression "$Python -m venv `"$venv`""
}

$venvPython = Join-Path $venv "Scripts\python.exe"
& $venvPython -m pip install --upgrade pip --index-url $IndexUrl
& $venvPython -m pip install -r (Join-Path $projectRoot "engines\asr\requirements-dev.txt") --index-url $IndexUrl
& $venvPython (Join-Path $projectRoot "engines\asr\server.py") --health --model-dir (Join-Path $projectRoot ".models\asr")

Write-Host "VoiceDesk ASR 环境已就绪：$venvPython"
