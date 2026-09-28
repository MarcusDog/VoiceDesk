from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

from voicedesk_asr.engine import Transcriber, engine_health
from voicedesk_asr.llm_engine import GenerateDocumentRequest, LocalDocumentGenerator
from voicedesk_asr.protocol import EventWriter, PROTOCOL_VERSION, TranscribeRequest


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="VoiceDesk ASR JSON-lines sidecar")
    parser.add_argument("--model-dir", type=Path, default=Path(os.environ.get("VOICEDESK_MODEL_DIR", "models")))
    parser.add_argument("--health", action="store_true")
    return parser.parse_args()


def main() -> int:
    if hasattr(sys.stdin, "reconfigure"):
        sys.stdin.reconfigure(encoding="utf-8")
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    if hasattr(sys.stderr, "reconfigure"):
        sys.stderr.reconfigure(encoding="utf-8")
    args = parse_args()
    writer = EventWriter()
    if args.health:
        writer.emit("health", **engine_health(args.model_dir))
        return 0

    writer.emit("ready", protocolVersion=PROTOCOL_VERSION, **engine_health(args.model_dir))
    line = sys.stdin.readline()
    if not line:
        writer.emit("error", code="EMPTY_REQUEST", message="没有收到转写请求。")
        return 2
    message_type = ""
    try:
        message = json.loads(line)
        message_type = str(message.get("type") or "")
        if message_type == "transcribe":
            request = TranscribeRequest.from_message(message)
            Transcriber(writer, args.model_dir).run(request)
        elif message_type == "generate_document":
            request = GenerateDocumentRequest.from_message(message)
            LocalDocumentGenerator(writer).run(request)
        else:
            raise ValueError("unsupported request type")
        return 0
    except Exception as error:
        writer.emit(
            "error",
            code=type(error).__name__.upper(),
            message="语音识别引擎执行失败。" if message_type == "transcribe" else "AI 文档引擎执行失败。",
            detail=str(error),
        )
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
