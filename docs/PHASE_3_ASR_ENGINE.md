# VoiceDesk 阶段三：本地 ASR Engine

更新时间：2026-09-27

## 已交付链路

```text
导入音视频
  → Rust Core 创建 Project / Media / Task
  → 启动独立 Python sidecar
  → PyAV 本地解码
  → faster-whisper + Silero VAD
  → JSON-lines 增量 Segment 事件
  → SQLite transcript_segments
  → Tauri Event 实时刷新工作区
```

音频、模型缓存和转写正文均保留在本机。ASR 不依赖系统 FFmpeg；未安装 FFprobe 时，Rust Core 会使用基础文件信息继续导入。FFmpeg 仍用于可选的 PCM 预处理与波形图片。

## 模型与硬件策略

| 产品模式 | 模型 | Beam | 目标 |
|---|---:|---:|---|
| 快速 | `base` | 3 | 快速浏览 |
| 自动推荐 | `small` | 5 | 默认质量/速度平衡 |
| 高精度 | `medium` | 5 | 更高中文准确率 |
| 多人会议 | `medium` | 5 | 先保证文字质量；说话人分离待接入 |

运行时先检测 CTranslate2 CUDA。GPU 可用时使用 `float16`；模型加载或推理失败会发出 `runtime_fallback` 事件并自动改用 CPU `int8`。当前测试设备为 RTX 3060 Laptop 6 GB、16 GB RAM。

## Sidecar 协议

Rust 向 stdin 写入一条 `transcribe` JSON 请求；sidecar 在 stdout 持续写入 `ready`、`model_loading`、`runtime_fallback`、`media_started`、`segment`、`progress`、`completed` 或 `error` 事件。

所有消息包含 `protocolVersion: 1`。stderr 只用于库日志和崩溃诊断，不参与业务协议。

## 本机验收结果

- 引擎：faster-whisper 1.2.1 / CTranslate2 4.8.2
- 模型：multilingual `small`
- 设备：CUDA `float16`
- 测试音频：9.356 秒 Windows 中文 TTS
- 推理时间：0.891 秒（模型已加载/缓存）
- 输出：2 个增量片段，时间戳、语言、置信度完整
- 文本：两句测试文本均准确识别

## 开发命令

```powershell
npm run asr:setup
npm run asr:test
npm run asr:build
npm run desktop:dev
```

`scripts/setup-asr.ps1` 默认只对本次 pip 调用使用清华镜像，不会修改全局 pip 配置。模型首次使用时下载到 `%APPDATA%/com.voicedesk.desktop/models/asr`。

## 已知边界

- 当前未启用说话人分离，所有 segment 暂显示为“说话人”。
- 任务暂停后使用“重新转写”从头安全执行，不做模型内部断点续推。
- `npm run desktop:build` 会检查并打包冻结后的 Python sidecar；最终用户不需要安装 Python。
