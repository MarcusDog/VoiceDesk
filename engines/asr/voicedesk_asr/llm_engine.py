from __future__ import annotations

import json
import os
import socket
import subprocess
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

from .protocol import EventWriter


PROMPT_VERSION = "structured-docs-v2"
MINIMAX_ENDPOINT = "https://api.minimaxi.com/v1/chat/completions"
MINIMAX_MODEL = "MiniMax-M3"

DOCUMENT_PROFILES: dict[str, dict[str, str]] = {
    "meeting_notes": {
        "label": "会议纪要",
        "title_suffix": "会议纪要",
        "summary": "用 2-4 句概括会议目标、进展和结果。",
        "sections": "conclusions 写明确结论；topics 写关键讨论；actionItems 写明确待办；risks 写原文明确的风险或阻塞。",
    },
    "executive_summary": {
        "label": "执行摘要",
        "title_suffix": "执行摘要",
        "summary": "用 3-5 句给出管理者可快速阅读的结果摘要，优先保留数字、决定和影响范围。",
        "sections": "conclusions 写核心结论；topics 写关键事实；actionItems 写下一步；risks 写需要关注的风险。",
    },
    "action_plan": {
        "label": "行动清单",
        "title_suffix": "行动清单",
        "summary": "简要说明目标、当前状态和执行重点。",
        "sections": "conclusions 写已确认的目标与决策；topics 写工作事项；actionItems 尽可能完整提取行动、负责人和期限；risks 写阻塞项。",
    },
    "interview_insights": {
        "label": "访谈洞察",
        "title_suffix": "访谈洞察",
        "summary": "概括受访者背景、核心诉求和主要反馈，不做样本外推。",
        "sections": "conclusions 写来源直接支持的核心洞察；topics 写主题与观点；actionItems 写明确的后续研究或验证事项；risks 写待验证问题和信息缺口。",
    },
}


DOCUMENT_SCHEMA: dict[str, Any] = {
    "type": "object",
    "properties": {
        "title": {"type": "string"},
        "summary": {"type": "string"},
        "conclusions": {
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "sourceSequences": {"type": "array", "items": {"type": "integer"}},
                },
                "required": ["text", "sourceSequences"],
                "additionalProperties": False,
            },
        },
        "topics": {
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "sourceSequences": {"type": "array", "items": {"type": "integer"}},
                },
                "required": ["text", "sourceSequences"],
                "additionalProperties": False,
            },
        },
        "actionItems": {
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "owner": {"type": ["string", "null"]},
                    "due": {"type": ["string", "null"]},
                    "sourceSequences": {"type": "array", "items": {"type": "integer"}},
                },
                "required": ["text", "owner", "due", "sourceSequences"],
                "additionalProperties": False,
            },
        },
        "risks": {
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "sourceSequences": {"type": "array", "items": {"type": "integer"}},
                },
                "required": ["text", "sourceSequences"],
                "additionalProperties": False,
            },
        },
    },
    "required": ["title", "summary", "conclusions", "topics", "actionItems", "risks"],
    "additionalProperties": False,
}


@dataclass(frozen=True)
class GenerateDocumentRequest:
    task_id: str
    project_title: str
    document_type: str
    segments: list[dict[str, Any]]
    provider: str
    llama_server: Path | None = None
    model_path: Path | None = None
    api_endpoint: str | None = None
    api_key: str | None = None
    model_name: str = MINIMAX_MODEL
    context_size: int = 12_288

    @classmethod
    def from_message(cls, message: dict[str, Any]) -> "GenerateDocumentRequest":
        if message.get("type") != "generate_document":
            raise ValueError("expected a generate_document request")
        task_id = str(message.get("taskId") or "").strip()
        segments = [item for item in message.get("segments", []) if str(item.get("text") or "").strip()]
        provider = str(message.get("provider") or "local").strip().lower()
        document_type = str(message.get("documentType") or "meeting_notes").strip()
        if not task_id:
            raise ValueError("taskId is required")
        if not segments:
            raise ValueError("at least one transcript segment is required")
        if document_type not in DOCUMENT_PROFILES:
            raise ValueError("unsupported document type")
        llama_server: Path | None = None
        model_path: Path | None = None
        api_endpoint: str | None = None
        api_key: str | None = None
        model_name = str(message.get("modelName") or MINIMAX_MODEL).strip()
        if provider == "minimax":
            api_endpoint = str(message.get("apiEndpoint") or "").strip()
            api_key = str(message.get("apiKey") or "").strip()
            if api_endpoint != MINIMAX_ENDPOINT:
                raise ValueError("unsupported MiniMax API endpoint")
            if model_name != MINIMAX_MODEL:
                raise ValueError("unsupported MiniMax model")
            if len(api_key) < 20:
                raise ValueError("MiniMax API key is missing")
        elif provider == "local":
            llama_server = Path(str(message.get("llamaServer") or ""))
            model_path = Path(str(message.get("modelPath") or ""))
            if not llama_server.is_file():
                raise FileNotFoundError(f"llama-server not found: {llama_server}")
            if not model_path.is_file():
                raise FileNotFoundError(f"GGUF model not found: {model_path}")
            model_name = model_path.name
        else:
            raise ValueError("unsupported document provider")
        return cls(
            task_id=task_id,
            project_title=str(message.get("projectTitle") or "语音记录").strip(),
            document_type=document_type,
            segments=segments,
            provider=provider,
            llama_server=llama_server,
            model_path=model_path,
            api_endpoint=api_endpoint,
            api_key=api_key,
            model_name=model_name,
            context_size=max(4_096, min(32_768, int(message.get("contextSize") or 12_288))),
        )


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as server:
        server.bind(("127.0.0.1", 0))
        return int(server.getsockname()[1])


def _post_json(
    url: str,
    payload: dict[str, Any],
    timeout: float,
    headers: dict[str, str] | None = None,
) -> dict[str, Any]:
    request_headers = {"Content-Type": "application/json; charset=utf-8"}
    request_headers.update(headers or {})
    request = urllib.request.Request(
        url,
        data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        headers=request_headers,
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.loads(response.read().decode("utf-8"))


def _wait_until_ready(port: int, process: subprocess.Popen[bytes], timeout: float = 120.0) -> None:
    deadline = time.monotonic() + timeout
    last_error = ""
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"llama-server exited with status {process.returncode}")
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=1.5) as response:
                value = json.loads(response.read().decode("utf-8"))
                if value.get("status") == "ok":
                    return
        except (OSError, ValueError, urllib.error.URLError) as error:
            last_error = str(error)
        time.sleep(0.35)
    raise TimeoutError(f"llama-server startup timed out: {last_error}")


def _chunk_lines(lines: Iterable[str], max_chars: int = 11_000) -> list[str]:
    chunks: list[str] = []
    current: list[str] = []
    size = 0
    for line in lines:
        if current and size + len(line) > max_chars:
            chunks.append("\n".join(current))
            current = []
            size = 0
        current.append(line)
        size += len(line) + 1
    if current:
        chunks.append("\n".join(current))
    return chunks


def _timestamp(milliseconds: int) -> str:
    seconds = max(0, milliseconds) // 1000
    return f"{seconds // 3600:02}:{seconds // 60 % 60:02}:{seconds % 60:02}"


def _extract_json(text: str) -> dict[str, Any]:
    value = text.strip()
    if value.startswith("```"):
        value = value.split("\n", 1)[-1]
        value = value.rsplit("```", 1)[0]
    start = value.find("{")
    end = value.rfind("}")
    if start < 0 or end < start:
        raise ValueError("model response did not contain a JSON object")
    return json.loads(value[start : end + 1])


def _normalize_document(document: dict[str, Any], valid_sequences: set[int], fallback_title: str) -> dict[str, Any]:
    def items(key: str, *, action: bool = False) -> list[dict[str, Any]]:
        result: list[dict[str, Any]] = []
        for item in document.get(key, []) if isinstance(document.get(key), list) else []:
            if not isinstance(item, dict) or not str(item.get("text") or "").strip():
                continue
            sequences = []
            for sequence in item.get("sourceSequences", []):
                try:
                    sequence = int(sequence)
                except (TypeError, ValueError):
                    continue
                if sequence in valid_sequences and sequence not in sequences:
                    sequences.append(sequence)
            normalized = {"text": str(item["text"]).strip(), "sourceSequences": sequences[:8]}
            if action:
                normalized["owner"] = str(item.get("owner") or "").strip() or None
                normalized["due"] = str(item.get("due") or "").strip() or None
            result.append(normalized)
        return result[:12]

    return {
        "title": str(document.get("title") or fallback_title).strip()[:120],
        "summary": str(document.get("summary") or "").strip(),
        "conclusions": items("conclusions"),
        "topics": items("topics"),
        "actionItems": items("actionItems", action=True),
        "risks": items("risks"),
    }


class LocalDocumentGenerator:
    def __init__(self, writer: EventWriter) -> None:
        self.writer = writer

    def run(self, request: GenerateDocumentRequest) -> None:
        if request.provider == "minimax":
            self.writer.emit("progress", progress=0.05, stage="正在安全连接 MiniMax-M3")
            self._generate(request, None)
            return

        port = _free_port()
        creation_flags = 0x08000000 if os.name == "nt" else 0
        command = [
            str(request.llama_server),
            "-m", str(request.model_path),
            "--host", "127.0.0.1",
            "--port", str(port),
            "--ctx-size", str(request.context_size),
            "--parallel", "1",
            "--gpu-layers", "99",
            "--no-webui",
            "--reasoning", "off",
        ]
        self.writer.emit("progress", progress=0.02, stage="正在加载本地语言模型")
        process = subprocess.Popen(
            command,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            creationflags=creation_flags,
        )
        try:
            _wait_until_ready(port, process)
            self.writer.emit("progress", progress=0.12, stage="本地语言模型已就绪")
            self._generate(request, port)
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)

    def _generate(self, request: GenerateDocumentRequest, port: int | None) -> None:
        profile = DOCUMENT_PROFILES[request.document_type]
        lines = [
            f"[S{int(segment.get('sequence', index))} {_timestamp(int(segment.get('startMs', 0)))}] "
            f"{str(segment.get('speaker') or '说话人')}: {str(segment.get('text') or '').strip()}"
            for index, segment in enumerate(request.segments)
        ]
        chunks = _chunk_lines(lines)
        source_text: str
        if len(chunks) == 1:
            source_text = chunks[0]
        else:
            notes: list[str] = []
            for index, chunk in enumerate(chunks):
                progress = 0.12 + (0.48 * index / len(chunks))
                self.writer.emit(
                    "progress",
                    progress=progress,
                    stage=f"正在整理第 {index + 1}/{len(chunks)} 个内容分段",
                )
                notes.append(self._summarize_chunk(request, port, chunk, index + 1))
            source_text = "\n\n".join(notes)

        self.writer.emit("progress", progress=0.64, stage=f"正在生成{profile['label']}")
        raw_document = self._create_document(request, port, source_text)
        self.writer.emit("progress", progress=0.86, stage="正在核对事实与来源")
        raw_document = self._audit_document(request, port, source_text, raw_document)
        valid_sequences = {int(item.get("sequence", index)) for index, item in enumerate(request.segments)}
        document = _normalize_document(raw_document, valid_sequences, f"{request.project_title}{profile['title_suffix']}")
        self.writer.emit(
            "document",
            document=document,
            model=request.model_name,
            promptVersion=PROMPT_VERSION,
        )
        self.writer.emit(
            "completed",
            progress=1.0,
            stage="AI 文档已生成",
            model=request.model_name,
            promptVersion=PROMPT_VERSION,
        )

    def _chat(
        self,
        request: GenerateDocumentRequest,
        port: int | None,
        messages: list[dict[str, str]],
        max_tokens: int,
        schema: dict[str, Any] | None = None,
    ) -> str:
        payload: dict[str, Any] = {
            "model": request.model_name if request.provider == "minimax" else "local",
            "messages": messages,
            "temperature": 0.1,
            "top_p": 0.85,
            "max_tokens": max_tokens,
            "stream": False,
        }
        if request.provider == "minimax":
            payload["max_completion_tokens"] = payload.pop("max_tokens")
            payload["thinking"] = {"type": "disabled"}
            response = _post_json(
                request.api_endpoint or "",
                payload,
                timeout=300,
                headers={"Authorization": f"Bearer {request.api_key}"},
            )
        else:
            if port is None:
                raise RuntimeError("local LLM port is missing")
            if schema is not None:
                payload["response_format"] = {
                    "type": "json_schema",
                    "json_schema": {"name": "voicedesk_document", "strict": True, "schema": schema},
                }
            response = _post_json(f"http://127.0.0.1:{port}/v1/chat/completions", payload, timeout=300)
        return str(response["choices"][0]["message"].get("content") or "")

    def _summarize_chunk(self, request: GenerateDocumentRequest, port: int | None, chunk: str, number: int) -> str:
        profile = DOCUMENT_PROFILES[request.document_type]
        return self._chat(
            request,
            port,
            [
                {
                    "role": "system",
                    "content": (
                        "你是忠实的会议记录整理器。只能使用输入中的事实；不得补充常识、猜测或虚构。"
                        "保留每条事实后对应的 [S数字] 来源标记。不明确的信息写“不明确”。"
                    ),
                },
                {
                    "role": "user",
                    "content": f"/no_think\n请为{profile['label']}压缩第 {number} 段转写，保留关键事实、行动项、风险和来源标记：\n{chunk}",
                },
            ],
            max_tokens=1_200,
        )

    def _create_document(
        self,
        request: GenerateDocumentRequest,
        port: int | None,
        source_text: str,
    ) -> dict[str, Any]:
        profile = DOCUMENT_PROFILES[request.document_type]
        system = (
            "你是 VoiceDesk 结构化文档引擎。只允许根据用户提供的转写或分段笔记生成内容。"
            "不得添加输入中不存在的人名、日期、数字、结论、因果关系或行动项。"
            "sourceSequences 必须引用输入里的 S 数字；没有证据就省略该条。风险只能复述原文明确提出的风险或阻塞，"
            "不得推断它可能造成的后果。"
            "owner 与 due 仅在原文明确出现时填写，否则必须为 null。输出严格 JSON，不要 Markdown。"
        )
        user = (
            f"/no_think\n项目：{request.project_title}\n文档类型：{profile['label']}\n"
            f"请生成中文结构化{profile['label']}。{profile['summary']}"
            f"{profile['sections']}\n\n"
            f"来源内容：\n{source_text}"
        )
        text = self._chat(
            request,
            port,
            [{"role": "system", "content": system}, {"role": "user", "content": user}],
            max_tokens=2_400,
            schema=DOCUMENT_SCHEMA,
        )
        return _extract_json(text)

    def _audit_document(
        self,
        request: GenerateDocumentRequest,
        port: int | None,
        source_text: str,
        draft: dict[str, Any],
    ) -> dict[str, Any]:
        system = (
            "你是严格的事实审校员。逐条对照来源审查草稿，只保留来源直接支持的最小事实陈述。"
            "删除常识补充、可能后果、因果推断、未出现的评价和不确定的责任人或期限。"
            "允许压缩与改写，但不能扩大原意。sourceSequences 必须准确。输出严格 JSON。"
        )
        user = (
            "/no_think\n请修订草稿，使每一句都能由来源直接证明。尤其不要为风险添加原文未说的影响。\n\n"
            f"来源：\n{source_text}\n\n草稿：\n{json.dumps(draft, ensure_ascii=False)}"
        )
        text = self._chat(
            request,
            port,
            [{"role": "system", "content": system}, {"role": "user", "content": user}],
            max_tokens=2_400,
            schema=DOCUMENT_SCHEMA,
        )
        return _extract_json(text)
