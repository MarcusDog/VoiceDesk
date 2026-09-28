# VoiceDesk v0.4.0 Personal Alpha 发布审计

日期：2026-09-27

## 本版结果

- Windows release 使用 GUI 子系统（PE Subsystem `2`），安装版启动不显示控制台窗口。
- 本地 faster-whisper 负责音频/录音转写；模型缓存已存在，首次缺失时优先使用镜像。
- MiniMax-M3 是 AI 文档主引擎，固定请求 `https://api.minimaxi.com/v1/chat/completions`。
- API 密钥仅写入 Windows 凭据管理器；前端不回读，源码、日志和工作区均不保存密钥。
- 本地 Qwen3-4B GGUF 继续作为断网备用，并复用现有 AppData 模型。
- 真实播放器、波形 seek、时间戳跳转、变速、静音、人工说话人命名和低置信度审核队列已接通；接受/忽略结果会持久化。
- 自动章节可在本机按时间轴生成并点击跳转；个人笔记与草稿自动保存到 SQLite。
- 项目使用软删除回收站，可恢复误删内容；运行中的任务不会被移入回收站。
- 产品范围已移除账号、团队空间、成员权限、分享审批与实时协作。

## 自动验证

| 项目 | 结果 |
|---|---|
| TypeScript | 通过 |
| Vite production build | 通过，1904 modules |
| Rust Core | 11 passed |
| Python ASR/LLM 协议 | 7 passed |
| API 密钥工作区扫描 | 未发现 MiniMax 密钥前缀或完整密钥 |
| Windows PE 子系统 | `Windows GUI` |

## 安装包

- 文件：`src-tauri/target/release/bundle/nsis/VoiceDesk_0.4.0_x64-setup.exe`
- 大小：84,763,123 字节
- SHA-256：`381BBDA513FA2D07EC50A4D596A1C3564B85510420B4DD7C595CEBC0F7554381`
- 签名：未签名，SmartScreen 仍可能提示未知发布者

## 尚需用户环境验收

1. 在 MiniMax 控制台撤销对话中暴露的旧密钥。
2. 安装 v0.4.0，在设置页保存新密钥并点击“测试连接”。
3. 录制一段真实语音，确认本地转写、媒体回放、时间戳跳转与编辑。
4. 主动生成一次 MiniMax-M3 会议纪要，核对来源跳转后导出。

24 项都已有实现落点，但完整度不同。深度功能与人工发布验收仍以 `IMPLEMENTATION_STATUS.md` 和 `.autonomous/voicedesk-personal-completion/task_list.md` 为准。
