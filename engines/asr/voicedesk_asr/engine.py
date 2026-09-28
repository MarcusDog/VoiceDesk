from __future__ import annotations

import math
import os
import platform
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Iterable, Protocol

from .protocol import EventWriter, TranscribeRequest


class SegmentLike(Protocol):
    start: float
    end: float
    text: str
    avg_logprob: float


ModelFactory = Callable[..., Any]
MODEL_REPOSITORIES = {
    "tiny": "Systran/faster-whisper-tiny",
    "base": "Systran/faster-whisper-base",
    "small": "Systran/faster-whisper-small",
    "medium": "Systran/faster-whisper-medium",
    "large-v3": "Systran/faster-whisper-large-v3",
}
REQUIRED_MODEL_FILES = ("config.json", "model.bin", "tokenizer.json", "vocabulary.txt")


@dataclass(frozen=True)
class RuntimeChoice:
    device: str
    compute_type: str


def choose_runtime(device: str, compute_type: str, cuda_devices: int) -> RuntimeChoice:
    requested = device.lower()
    if requested == "cpu" or cuda_devices < 1:
        return RuntimeChoice("cpu", "int8" if compute_type == "auto" else compute_type)
    return RuntimeChoice("cuda", "float16" if compute_type == "auto" else compute_type)


def confidence_from_logprob(avg_logprob: float | None) -> float | None:
    if avg_logprob is None or not math.isfinite(avg_logprob):
        return None
    return round(max(0.0, min(1.0, math.exp(avg_logprob))), 4)


def _cuda_device_count() -> int:
    try:
        import ctranslate2

        return int(ctranslate2.get_cuda_device_count())
    except Exception:
        return 0


def _load_factory() -> ModelFactory:
    from faster_whisper import WhisperModel

    return WhisperModel


def find_local_model_snapshot(download_root: Path, model: str) -> Path | None:
    direct = Path(model)
    if direct.is_dir() and all((direct / name).is_file() and (direct / name).stat().st_size > 0 for name in REQUIRED_MODEL_FILES):
        return direct
    repository = MODEL_REPOSITORIES.get(model, f"Systran/faster-whisper-{model}")
    snapshots = download_root / f"models--{repository.replace('/', '--')}" / "snapshots"
    if not snapshots.is_dir():
        return None
    candidates = sorted((path for path in snapshots.iterdir() if path.is_dir()), key=lambda path: path.stat().st_mtime, reverse=True)
    for candidate in candidates:
        if all((candidate / name).is_file() and (candidate / name).stat().st_size > 0 for name in REQUIRED_MODEL_FILES):
            return candidate
    return None


def remove_zero_length_model_files(download_root: Path, model: str) -> list[Path]:
    repository = MODEL_REPOSITORIES.get(model, f"Systran/faster-whisper-{model}")
    snapshots = download_root / f"models--{repository.replace('/', '--')}" / "snapshots"
    removed: list[Path] = []
    if not snapshots.is_dir():
        return removed
    for snapshot in snapshots.iterdir():
        if not snapshot.is_dir():
            continue
        for name in REQUIRED_MODEL_FILES:
            file = snapshot / name
            if not file.exists() or file.stat().st_size > 0:
                continue
            target = file.resolve(strict=False)
            if file.is_symlink():
                file.unlink(missing_ok=True)
            if target.exists() and target.is_file() and target.stat().st_size == 0:
                target.unlink(missing_ok=True)
            elif file.exists():
                file.unlink(missing_ok=True)
            removed.append(file)
    return removed


class Transcriber:
    def __init__(
        self,
        writer: EventWriter,
        download_root: Path,
        model_factory: ModelFactory | None = None,
        cuda_device_count: Callable[[], int] = _cuda_device_count,
    ) -> None:
        self.writer = writer
        self.download_root = download_root
        self.model_factory = model_factory or _load_factory()
        self.cuda_device_count = cuda_device_count

    def _load_model(self, request: TranscribeRequest, choice: RuntimeChoice) -> Any:
        local_snapshot = find_local_model_snapshot(self.download_root, request.model)
        if local_snapshot is None:
            remove_zero_length_model_files(self.download_root, request.model)
        self.writer.emit(
            "model_loading",
            taskId=request.task_id,
            model=request.model,
            device=choice.device,
            computeType=choice.compute_type,
            cached=local_snapshot is not None,
        )
        self.writer.emit(
            "progress",
            taskId=request.task_id,
            progress=0.01,
            stage="正在加载本机识别模型" if local_snapshot else "正在下载识别模型（仅首次使用）",
        )
        return self.model_factory(
            str(local_snapshot) if local_snapshot else request.model,
            device=choice.device,
            compute_type=choice.compute_type,
            download_root=str(self.download_root),
            local_files_only=local_snapshot is not None,
        )

    def run(self, request: TranscribeRequest) -> None:
        for media_path in request.media_paths:
            if not Path(media_path).is_file():
                raise FileNotFoundError(f"media file not found: {media_path}")

        self.download_root.mkdir(parents=True, exist_ok=True)
        cuda_devices = self.cuda_device_count()
        choice = choose_runtime(request.device, request.compute_type, cuda_devices)
        fallback_reason: str | None = None
        try:
            model = self._load_model(request, choice)
        except Exception as error:
            if choice.device != "cuda":
                raise
            fallback_reason = str(error)
            choice = RuntimeChoice("cpu", "int8")
            self.writer.emit(
                "runtime_fallback",
                taskId=request.task_id,
                fromDevice="cuda",
                toDevice="cpu",
                reason=fallback_reason,
            )
            model = self._load_model(request, choice)

        sequence = 0
        timeline_offset = 0.0
        total_files = len(request.media_paths)
        started = time.monotonic()
        detected_languages: list[str] = []

        for file_index, media_path in enumerate(request.media_paths):
            try:
                segments, info = self._transcribe(model, request, media_path)
                sequence, duration, language = self._consume_segments(
                    request,
                    segments,
                    info,
                    file_index,
                    total_files,
                    sequence,
                    timeline_offset,
                )
            except Exception as error:
                if choice.device != "cuda":
                    raise
                fallback_reason = str(error)
                choice = RuntimeChoice("cpu", "int8")
                self.writer.emit(
                    "runtime_fallback",
                    taskId=request.task_id,
                    fromDevice="cuda",
                    toDevice="cpu",
                    reason=fallback_reason,
                )
                model = self._load_model(request, choice)
                segments, info = self._transcribe(model, request, media_path)
                sequence, duration, language = self._consume_segments(
                    request,
                    segments,
                    info,
                    file_index,
                    total_files,
                    sequence,
                    timeline_offset,
                )
            timeline_offset += duration
            if language:
                detected_languages.append(language)

        self.writer.emit(
            "completed",
            taskId=request.task_id,
            segmentCount=sequence,
            durationMs=round(timeline_offset * 1000),
            elapsedMs=round((time.monotonic() - started) * 1000),
            model=request.model,
            device=choice.device,
            computeType=choice.compute_type,
            language=detected_languages[0] if detected_languages else request.language,
            fallbackReason=fallback_reason,
        )

    @staticmethod
    def _transcribe(model: Any, request: TranscribeRequest, media_path: str) -> tuple[Iterable[SegmentLike], Any]:
        return model.transcribe(
            media_path,
            language=request.language,
            task="transcribe",
            beam_size=request.beam_size,
            vad_filter=True,
            vad_parameters={"min_silence_duration_ms": 500, "speech_pad_ms": 300},
            word_timestamps=False,
            condition_on_previous_text=True,
            initial_prompt=request.initial_prompt,
        )

    def _consume_segments(
        self,
        request: TranscribeRequest,
        segments: Iterable[SegmentLike],
        info: Any,
        file_index: int,
        total_files: int,
        sequence: int,
        timeline_offset: float,
    ) -> tuple[int, float, str | None]:
        duration = float(getattr(info, "duration", 0.0) or 0.0)
        language = getattr(info, "language", None)
        probability = getattr(info, "language_probability", None)
        self.writer.emit(
            "media_started",
            taskId=request.task_id,
            fileIndex=file_index,
            path=request.media_paths[file_index],
            durationMs=round(duration * 1000),
            language=language,
            languageProbability=probability,
        )
        for segment in segments:
            text = str(segment.text).strip()
            if not text:
                continue
            start = timeline_offset + float(segment.start)
            end = timeline_offset + float(segment.end)
            self.writer.emit(
                "segment",
                taskId=request.task_id,
                sequence=sequence,
                startMs=round(start * 1000),
                endMs=max(round(end * 1000), round(start * 1000)),
                text=text,
                confidence=confidence_from_logprob(getattr(segment, "avg_logprob", None)),
                language=language,
                model=request.model,
                chunkId=f"{file_index}:{sequence}",
            )
            sequence += 1
            within_file = min(1.0, float(segment.end) / duration) if duration > 0 else 0.0
            progress = (file_index + within_file) / total_files
            self.writer.emit(
                "progress",
                taskId=request.task_id,
                progress=round(progress, 4),
                stage=f"正在识别第 {file_index + 1}/{total_files} 个文件",
            )
        return sequence, duration, language


def engine_health(download_root: Path) -> dict[str, object]:
    installed = False
    version = None
    try:
        import faster_whisper

        installed = True
        version = getattr(faster_whisper, "__version__", None)
    except Exception:
        pass
    return {
        "python": platform.python_version(),
        "platform": platform.platform(),
        "engineInstalled": installed,
        "engineVersion": version,
        "cudaDeviceCount": _cuda_device_count(),
        "modelDirectory": str(download_root),
        "pid": os.getpid(),
    }
