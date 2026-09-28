$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$binary = Join-Path $projectRoot "dist-asr\voicedesk-asr\voicedesk-asr.exe"
$sources = @(
  (Join-Path $projectRoot "engines\asr\server.py"),
  (Join-Path $projectRoot "engines\asr\voicedesk_asr\engine.py"),
  (Join-Path $projectRoot "engines\asr\voicedesk_asr\llm_engine.py"),
  (Join-Path $projectRoot "engines\asr\voicedesk_asr\protocol.py"),
  (Join-Path $projectRoot "engines\asr\requirements.txt")
)

$needsBuild = -not (Test-Path $binary)
if (-not $needsBuild) {
  $binaryTime = (Get-Item -LiteralPath $binary).LastWriteTimeUtc
  $needsBuild = $sources | Where-Object { (Get-Item -LiteralPath $_).LastWriteTimeUtc -gt $binaryTime } | Select-Object -First 1
}

if ($needsBuild) {
  & (Join-Path $PSScriptRoot "build-asr-sidecar.ps1")
} else {
  Write-Host "ASR sidecar is up to date."
}
