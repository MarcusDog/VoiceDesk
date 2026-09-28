# VoiceDesk 第四阶段：本地 LLM 文档引擎

版本：v0.3
状态：核心链路完成
更新日期：2026-09-27

## 目标与选型

VoiceDesk 使用 `Qwen3-4B-Q4_K_M.gguf` 与 llama.cpp Vulkan 后端，在 Windows 本机把转写整理成会议摘要、结论、讨论、待办和风险。模型正文不会上传云端。

当前开发机（RTX 3060 Laptop 6 GB）实测约 26 token/s。模型约 2.5 GB，原生上下文为 32K；应用默认使用 12K 上下文，以控制显存与内存占用。

## 流水线

```text
TranscriptSegment[]
  → 带 S 序号和时间戳的来源行
  → 约 11K 字符分段
  → 长内容分段压缩（保留 [S序号]）
  → 严格 JSON Schema 文档生成
  → 第二遍事实审校（删除无来源推断）
  → Rust 校验来源序号
  → ai_documents + source_refs 原子落库
```

短转写直接进入文档生成；长转写先分段压缩，因此不会把任意长度文本一次性塞入上下文。

## 忠实性约束

- 系统提示明确禁止添加人名、日期、数字、结论、因果和行动项。
- `owner` 与 `due` 只有原文明确出现时才允许填写。
- 风险只允许复述原文，禁止推断“可能导致”的后果。
- 生成后运行独立事实审校，再由 Python 丢弃不存在的 segment 序号。
- Rust 根据 sequence 解析真实 segment ID，并保存到 `source_refs`。
- 每份文档保存 `model` 与 `prompt_version`，当前版本为 `meeting-notes-v1`。

真实烟测中，第一轮曾把“设备未到货”扩写成“可能导致发布延期”；加入事实审校后结果被收敛为“测试设备尚未到货”。该用例保留为忠实性回归基准。

## 运行时与下载

- llama.cpp Vulkan DLL 随 NSIS 安装包分发。
- GGUF 不塞入安装器，首次可在“设置 → 文档生成模型”下载。
- 下载器先访问 Hugging Face 官方源；连接失败时自动切换 `hf-mirror.com`。
- 官方源单次允许最多 10 分钟；仍未完成时保留已下载字节并切换镜像续传，镜像单次允许最多 2 小时，避免慢速大文件被通用短超时反复打断。
- 下载写入 `.part`，支持 HTTP Range 断点续传；完成后校验固定 SHA-256，再原子重命名。
- 当前固定模型大小为 `2,497,280,256` 字节，SHA-256 为 `7485FE6F11AF29433BC51CAB58009521F205840F5B4AE3A32FA7F92E8534FDF5`；异常超长或哈希错误的断点文件会被丢弃，网络中断则保留供下次续传。
- 开发环境也可以运行 `npm run llm:setup`。

## IPC 与事件

命令：`llm_health`、`download_llm_model`、`generate_document`、`cancel_document_generation`、`list_ai_documents`。

事件：`core://llm-download-progress`、`core://task-updated`、`core://document-created`、`core://document-error`。

生成任务由 Rust 管理进程树；取消时会终止 sidecar 及其 llama-server 子进程。异常退出的 running 任务在下一次启动时恢复为 paused。

## 已验证

- Python LLM 工具测试 3 项通过。
- Rust Core 测试包含 AI 文档与 source_refs 持久化。
- 源码 sidecar 与 PyInstaller 冻结 sidecar 均完成真实 Qwen 推理。
- TypeScript、Vite 生产构建和 Rust 测试通过。
- 2026-09-27 再次用冻结 sidecar 生成真实结构化纪要；“测试设备未到”仅被复述为风险，没有推断发布延期。
