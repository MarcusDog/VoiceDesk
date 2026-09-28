from __future__ import annotations

import io
import json
from dataclasses import dataclass
from pathlib import Path

from voicedesk_asr.engine import (
    REQUIRED_MODEL_FILES,
    RuntimeChoice,
    Transcriber,
    choose_runtime,
    confidence_from_logprob,
    find_local_model_snapshot,
    remove_zero_length_model_files,
)
from voicedesk_asr.protocol import EventWriter, TranscribeRequest, normalize_language


@dataclass
class FakeSegment:
    start: float
    end: float
    text: str
    avg_logprob: float


@dataclass
class FakeInfo:
    duration: float = 3.0
    language: str = "zh"
    language_probability: float = 0.98


class FakeModel:
    def transcribe(self, _path: str, **_kwargs: object):
        return iter([
            FakeSegment(0.0, 1.2, " 大家好 ", -0.1),
            FakeSegment(1.5, 3.0, "开始开会。", -0.2),
        ]), FakeInfo()


def test_protocol_language_and_runtime() -> None:
    assert normalize_language("zh-CN") == "zh"
    assert normalize_language("auto") is None
    assert choose_runtime("auto", "auto", 1).device == "cuda"
    assert choose_runtime("auto", "auto", 0).compute_type == "int8"
    assert confidence_from_logprob(-0.1) == 0.9048


def test_transcriber_emits_incremental_segments(tmp_path: Path) -> None:
    media = tmp_path / "sample.wav"
    media.write_bytes(b"fake")
    stream = io.StringIO()
    transcriber = Transcriber(
        EventWriter(stream),
        tmp_path / "models",
        model_factory=lambda *_args, **_kwargs: FakeModel(),
        cuda_device_count=lambda: 0,
    )
    transcriber.run(TranscribeRequest(task_id="task-1", media_paths=[str(media)]))
    events = [json.loads(line) for line in stream.getvalue().splitlines()]
    segments = [event for event in events if event["type"] == "segment"]
    assert [segment["text"] for segment in segments] == ["大家好", "开始开会。"]
    assert events[-1]["type"] == "completed"
    assert events[-1]["segmentCount"] == 2


def test_complete_cached_model_is_loaded_without_network(tmp_path: Path) -> None:
    snapshot = tmp_path / "models--Systran--faster-whisper-small" / "snapshots" / "revision"
    snapshot.mkdir(parents=True)
    for name in REQUIRED_MODEL_FILES:
        (snapshot / name).write_bytes(b"ready")
    calls: list[tuple[object, dict[str, object]]] = []

    def factory(model: object, **kwargs: object) -> FakeModel:
        calls.append((model, kwargs))
        return FakeModel()

    stream = io.StringIO()
    transcriber = Transcriber(EventWriter(stream), tmp_path, model_factory=factory, cuda_device_count=lambda: 0)
    request = TranscribeRequest(task_id="task-cache", media_paths=[str(tmp_path / "unused.wav")], model="small")
    transcriber._load_model(request, RuntimeChoice("cpu", "int8"))

    assert find_local_model_snapshot(tmp_path, "small") == snapshot
    assert calls[0][0] == str(snapshot)
    assert calls[0][1]["local_files_only"] is True
    assert "正在加载本机识别模型" in stream.getvalue()


def test_zero_byte_model_is_rejected_and_removed_for_redownload(tmp_path: Path) -> None:
    snapshot = tmp_path / "models--Systran--faster-whisper-small" / "snapshots" / "revision"
    snapshot.mkdir(parents=True)
    for name in REQUIRED_MODEL_FILES:
        (snapshot / name).write_bytes(b"" if name == "model.bin" else b"ready")

    assert find_local_model_snapshot(tmp_path, "small") is None
    removed = remove_zero_length_model_files(tmp_path, "small")
    assert snapshot / "model.bin" in removed
    assert not (snapshot / "model.bin").exists()
