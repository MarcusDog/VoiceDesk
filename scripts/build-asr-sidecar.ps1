param(
  [string]$IndexUrl = "https://pypi.tuna.tsinghua.edu.cn/simple"
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$python = Join-Path $projectRoot ".venv-asr\Scripts\python.exe"
if (-not (Test-Path $python)) {
  throw "ASR virtual environment not found. Run npm run asr:setup first."
}

& $python -m pip install -r (Join-Path $projectRoot "engines\asr\requirements-build.txt") --index-url $IndexUrl
& $python -m PyInstaller `
  --noconfirm `
  --clean `
  --onedir `
  --name voicedesk-asr `
  --collect-data faster_whisper `
  --exclude-module pytest `
  --exclude-module py `
  --paths (Join-Path $projectRoot "engines\asr") `
  --distpath (Join-Path $projectRoot "dist-asr") `
  --workpath (Join-Path $projectRoot ".build-asr") `
  --specpath (Join-Path $projectRoot ".build-asr") `
  (Join-Path $projectRoot "engines\asr\server.py")

$binary = Join-Path $projectRoot "dist-asr\voicedesk-asr\voicedesk-asr.exe"
& $binary --health --model-dir (Join-Path $projectRoot ".models\asr")
Write-Host "VoiceDesk ASR sidecar built: $binary"
