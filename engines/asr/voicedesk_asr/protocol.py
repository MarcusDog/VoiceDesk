from __future__ import annotations

import json
import sys
from dataclasses import asdict, dataclass
from typing import Any, TextIO


PROTOCOL_VERSION = 1


@dataclass(frozen=True)
class TranscribeRequest:
    task_id: str
    media_paths: list[str]
    model: str = "small"
    language: str | None = "zh"
    device: str = "auto"
    compute_type: str = "auto"
    beam_size: int = 5
    initial_prompt: str | None = None

    @classmethod
    def from_message(cls, message: dict[str, Any]) -> "TranscribeRequest":
        if message.get("type") != "transcribe":
            raise ValueError("expected a transcribe request")
        task_id = str(message.get("taskId", "")).strip()
        media_paths = [str(path) for path in message.get("mediaPaths", []) if str(path).strip()]
        if not task_id:
            raise ValueError("taskId is required")
        if not media_paths:
            raise ValueError("at least one media path is required")
        return cls(
            task_id=task_id,
            media_paths=media_paths,
            model=str(message.get("model") or "small"),
            language=normalize_language(message.get("language")),
            device=str(message.get("device") or "auto"),
            compute_type=str(message.get("computeType") or "auto"),
            beam_size=max(1, min(10, int(message.get("beamSize") or 5))),
            initial_prompt=str(message["initialPrompt"]).strip() if message.get("initialPrompt") else None,
        )


def normalize_language(value: object) -> str | None:
    if value is None:
        return None
    language = str(value).strip().lower()
    if language in {"", "auto", "自动检测"}:
        return None
    return {"zh-cn": "zh", "zh-hans": "zh", "en-us": "en", "en-gb": "en"}.get(language, language)


class EventWriter:
    def __init__(self, stream: TextIO | None = None) -> None:
        self._stream = stream or sys.stdout

    def emit(self, event_type: str, **payload: Any) -> None:
        message = {"protocolVersion": PROTOCOL_VERSION, "type": event_type, **payload}
        self._stream.write(json.dumps(message, ensure_ascii=False, separators=(",", ":")) + "\n")
        self._stream.flush()

    def emit_dataclass(self, event_type: str, value: object) -> None:
        self.emit(event_type, **asdict(value))
