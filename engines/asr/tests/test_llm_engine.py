import io
import json

import pytest

from voicedesk_asr.llm_engine import (
    DOCUMENT_PROFILES,
    MINIMAX_ENDPOINT,
    GenerateDocumentRequest,
    LocalDocumentGenerator,
    _chunk_lines,
    _extract_json,
    _normalize_document,
)
from voicedesk_asr.protocol import EventWriter


def test_chunk_lines_preserves_all_input() -> None:
    lines = ["a" * 6, "b" * 6, "c" * 6]
    chunks = _chunk_lines(lines, max_chars=10)
    assert chunks == ["a" * 6, "b" * 6, "c" * 6]


def test_extract_json_accepts_fenced_response() -> None:
    assert _extract_json('```json\n{"title":"会议"}\n```')["title"] == "会议"


def test_normalize_document_removes_unknown_sources() -> None:
    result = _normalize_document(
        {
            "title": "纪要",
            "summary": "摘要",
            "conclusions": [{"text": "结论", "sourceSequences": [1, 99]}],
            "topics": [],
            "actionItems": [{"text": "执行", "owner": "小王", "due": "周五", "sourceSequences": [2]}],
            "risks": [],
        },
        {1, 2},
        "默认标题",
    )
    assert result["conclusions"][0]["sourceSequences"] == [1]
    assert result["actionItems"][0]["owner"] == "小王"


def test_minimax_request_accepts_only_fixed_endpoint_and_model() -> None:
    request = GenerateDocumentRequest.from_message(
        {
            "type": "generate_document",
            "taskId": "task-1",
            "projectTitle": "个人记录",
            "provider": "minimax",
            "apiEndpoint": MINIMAX_ENDPOINT,
            "apiKey": "test-key-long-enough-for-validation",
            "modelName": "MiniMax-M3",
            "segments": [{"sequence": 0, "text": "需要完成语音转写。"}],
        }
    )
    assert request.provider == "minimax"
    assert request.model_name == "MiniMax-M3"
    assert request.llama_server is None


def test_document_templates_are_validated() -> None:
    request = GenerateDocumentRequest.from_message(
        {
            "type": "generate_document",
            "taskId": "task-template",
            "projectTitle": "用户访谈",
            "documentType": "interview_insights",
            "provider": "minimax",
            "apiEndpoint": MINIMAX_ENDPOINT,
            "apiKey": "test-key-long-enough-for-validation",
            "modelName": "MiniMax-M3",
            "segments": [{"sequence": 0, "text": "用户希望搜索更快。"}],
        }
    )
    assert request.document_type == "interview_insights"
    assert DOCUMENT_PROFILES[request.document_type]["label"] == "访谈洞察"

    with pytest.raises(ValueError, match="document type"):
        GenerateDocumentRequest.from_message(
            {
                "type": "generate_document",
                "taskId": "task-template",
                "documentType": "unknown",
                "provider": "minimax",
                "apiEndpoint": MINIMAX_ENDPOINT,
                "apiKey": "test-key-long-enough-for-validation",
                "modelName": "MiniMax-M3",
                "segments": [{"sequence": 0, "text": "内容"}],
            }
        )


def test_action_plan_profile_changes_generation_prompt() -> None:
    captured: list[list[dict[str, str]]] = []

    class StubGenerator(LocalDocumentGenerator):
        def _chat(self, request, port, messages, max_tokens, schema=None):  # type: ignore[no-untyped-def]
            captured.append(messages)
            return json.dumps(
                {
                    "title": "行动计划",
                    "summary": "执行重点",
                    "conclusions": [],
                    "topics": [],
                    "actionItems": [],
                    "risks": [],
                },
                ensure_ascii=False,
            )

    request = GenerateDocumentRequest.from_message(
        {
            "type": "generate_document",
            "taskId": "task-action",
            "projectTitle": "产品周会",
            "documentType": "action_plan",
            "provider": "minimax",
            "apiEndpoint": MINIMAX_ENDPOINT,
            "apiKey": "test-key-long-enough-for-validation",
            "modelName": "MiniMax-M3",
            "segments": [{"sequence": 0, "startMs": 0, "text": "小王周五完成测试。"}],
        }
    )
    generator = StubGenerator(EventWriter(io.StringIO()))
    generator._create_document(request, None, "[S0] 小王周五完成测试。")
    prompt = "\n".join(message["content"] for message in captured[0])
    assert "文档类型：行动清单" in prompt
    assert "负责人和期限" in prompt


def test_minimax_request_rejects_untrusted_endpoint() -> None:
    with pytest.raises(ValueError, match="endpoint"):
        GenerateDocumentRequest.from_message(
            {
                "type": "generate_document",
                "taskId": "task-1",
                "provider": "minimax",
                "apiEndpoint": "https://example.invalid/v1/chat/completions",
                "apiKey": "test-key-long-enough-for-validation",
                "modelName": "MiniMax-M3",
                "segments": [{"sequence": 0, "text": "内容"}],
            }
        )
