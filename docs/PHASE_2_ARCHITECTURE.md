# VoiceDesk 第二阶段技术架构

版本：v0.2
状态：基础工程完成
更新日期：2026-09-26

## 1. 阶段目标

阶段二把阶段一的 React 产品原型升级为真实 Windows 桌面应用基础：界面仍由 React 渲染，所有本地数据、文件、任务和媒体工具调用统一经过 Rust Core。前端不直接调用 Python，也不直接写入 SQLite。

```text
React UI
   │ invoke / event
   ▼
Rust Core
   ├── SQLite Repository
   ├── Recoverable Task State Machine
   ├── Native File Dialog
   ├── Media Probe / PCM / Waveform
   ├── Diagnostics & Logging
   ├── ASR / LLM Sidecar Process Manager
   └── Model Download & Integrity Verification
```

## 2. 目录与职责

| 路径 | 职责 |
|---|---|
| `src/core/client.ts` | 前端 IPC 客户端、浏览器/Tauri 运行时降级 |
| `src/core/types.ts` | 与 Rust 序列化结构对应的 TypeScript 类型 |
| `src-tauri/src/commands.rs` | Tauri command 与事件出口 |
| `src-tauri/src/database.rs` | SQLite 连接、仓储和迁移入口 |
| `src-tauri/src/tasks.rs` | 任务状态转换规则 |
| `src-tauri/src/media.rs` | FFprobe、PCM 转码、波形缓存 |
| `src-tauri/src/asr.rs` | ASR sidecar 生命周期、事件与增量入库 |
| `src-tauri/src/llm.rs` | llama.cpp、模型下载校验、AI 文档任务 |
| `src-tauri/src/system.rs` | CPU、内存、磁盘与工具可用性检测 |
| `src-tauri/src/error.rs` | 可序列化、用户可读的错误契约 |
| `src-tauri/migrations/001_initial.sql` | 第一版完整数据结构 |

## 3. IPC 命令

| 命令 | 输入 | 输出 |
|---|---|---|
| `core_health` | 无 | 核心版本、数据库路径、项目/活动任务数量 |
| `list_projects` | 无 | 项目列表 |
| `create_project` | 标题、语言 | 新项目 |
| `probe_media` | 文件路径 | 格式、时长、大小、采样率、声道、视频标记 |
| `register_media` | 项目 ID、媒体信息 | 媒体 ID |
| `prepare_audio` | 文件路径 | 16 kHz 单声道 PCM WAV 缓存路径 |
| `cache_waveform` | 文件路径 | 波形 PNG 缓存路径 |
| `enqueue_task` | 类型、项目、JSON 参数 | 新任务 |
| `transition_task` | 任务 ID、目标状态、阶段、进度 | 更新后的任务 |
| `list_tasks` | 无 | 最近 100 个任务 |
| `start_transcription` / `cancel_transcription` | 任务 ID | ASR 任务状态 |
| `llm_health` / `download_llm_model` | 无 | 本地 LLM 状态 / 模型路径 |
| `generate_document` / `cancel_document_generation` | 项目 ID / 任务 ID | LLM 任务状态 |
| `list_ai_documents` | 无 | 已生成 AI 文档和来源 |
| `replace_transcript_text` | 项目、查找、替换 | 被修改片段数 |
| `system_snapshot` | 无 | 系统与媒体工具检测结果 |
| `export_diagnostics` | 无 | 不包含用户内容的诊断报告路径 |

任务变化通过 `core://task-updated` 事件广播给前端。

## 4. SQLite 数据模型

- `projects`：一条声音工作记录。
- `media`：原始媒体、探测信息和缓存引用。
- `tasks`：长任务状态、进度、重试与恢复信息。
- `speakers`：项目内说话人身份。
- `transcript_segments`：时间轴、原文、编辑稿、置信度和 chunk 关联。
- `ai_documents`：AI 文档正文、模型与 prompt 版本。
- `source_refs`：AI 文档块到原始转写片段的可追溯关系。

SQLite 使用 WAL、外键约束和 5 秒 busy timeout。迁移在 Core 启动时执行，失败则阻止应用进入不一致状态。

## 5. 任务状态机

```text
queued ──▶ running ──▶ completed
  │           │  ├──▶ failed ──▶ queued (retry)
  │           │  ├──▶ paused ──▶ running
  └──────────▶ cancelled ◀──────┘
```

非法状态转换返回 `INVALID_TASK_TRANSITION`。应用启动时，遗留的 `running` 任务会改为 `paused` 并标记 `recovery_available = 1`，避免崩溃后假装任务仍在执行。

## 6. 本地目录

Tauri 根据应用标识 `com.voicedesk.desktop` 解析用户目录：

```text
AppData/Roaming/com.voicedesk.desktop/
  voicedesk.db
  projects/
  models/asr/
  models/llm/
  diagnostics/

AppData/Local/com.voicedesk.desktop/
  waveforms/
  audio/
```

不使用硬编码用户名或绝对路径。

## 7. 媒体处理

- `ffprobe` 读取容器格式、时长、文件大小、采样率、声道和视频流。
- `ffmpeg` 将输入统一转换为 16 kHz、单声道、16-bit PCM WAV。
- 波形缓存生成 1200×180 PNG。
- 输出先写 `.partial`，成功后原子重命名，避免中断留下看似完整的缓存。
- 缓存键包含路径、文件大小和修改时间。
- 工具缺失时返回 `FFMPEG_NOT_FOUND` / `FFPROBE_NOT_FOUND`，界面不显示底层命令行错误。

当前开发机未安装 FFmpeg，所以只完成了接口、检测与失败路径验收；安装后无需修改代码即可启用。

## 8. 错误与诊断

IPC 错误统一为：

```json
{
  "code": "FFPROBE_NOT_FOUND",
  "message": "尚未找到媒体分析组件……",
  "detail": "仅用于诊断的底层错误"
}
```

诊断报告只包含核心版本、任务数量、系统资源和工具可用性，不包含音频、文件内容、转写正文或 AI 文档。

## 9. 验收结果

- React TypeScript 检查通过。
- Vite 生产构建通过。
- Rust `cargo check` 通过。
- 6 个 Rust 单元测试通过（含 AI 文档来源持久化）。
- Tauri Windows 开发窗口启动成功。
- AppData 目录、SQLite 数据库和模型目录实际创建成功。

后续 ASR 与 LLM 实现分别记录在第三、第四阶段文档；完整页面状态见 `IMPLEMENTATION_STATUS.md`。
