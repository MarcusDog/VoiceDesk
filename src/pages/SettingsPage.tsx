import { Cloud, Cpu, Database, Download, HardDrive, KeyRound, Languages, ShieldCheck, Trash2, Type } from 'lucide-react'
import { useEffect, useState } from 'react'
import { deleteMinimaxApiKey, downloadLlmModel, exportDiagnostics, getAiProviderStatus, getAsrHealth, getLlmHealth, getSystemSnapshot, onLlmDownloadProgress, saveMinimaxApiKey, testMinimaxConnection } from '../core/client'
import type { AiProviderStatus, AsrHealth, CoreConnection, CoreError, LlmHealth, SystemSnapshot } from '../core/types'

type SettingsPageProps = { core: CoreConnection }
const formatGigabytes = (bytes?: number) => bytes ? `${(bytes / 1024 ** 3).toFixed(1)} GB` : '检测中…'
const unavailable = '该管理功能尚未开放'
type UiScale = '1' | '1.1' | '1.25'
const UI_SCALE_KEY = 'voicedesk:ui-scale'
const readUiScale = (): UiScale => {
  const value = localStorage.getItem(UI_SCALE_KEY)
  return value === '1' || value === '1.25' ? value : '1.1'
}
const applyUiScale = (value: UiScale) => {
  const scale = Number(value)
  const root = document.documentElement
  root.style.setProperty('--ui-zoom', value)
  root.style.setProperty('--ui-width', `${100 / scale}vw`)
  root.style.setProperty('--ui-flow-width', `${100 / scale}%`)
  root.style.setProperty('--ui-height', `${100 / scale}vh`)
}
applyUiScale(readUiScale())

export function SettingsPage({ core }: SettingsPageProps) {
  const [system, setSystem] = useState<SystemSnapshot | null>(null)
  const [asr, setAsr] = useState<AsrHealth | null>(null)
  const [llm, setLlm] = useState<LlmHealth | null>(null)
  const [provider, setProvider] = useState<AiProviderStatus | null>(null)
  const [apiKey, setApiKey] = useState('')
  const [savingKey, setSavingKey] = useState(false)
  const [diagnosticMessage, setDiagnosticMessage] = useState('')
  const [downloadProgress, setDownloadProgress] = useState<{ progress: number; stage: string }>()
  const [uiScale, setUiScale] = useState<UiScale>(readUiScale)

  const changeUiScale = (value: UiScale) => {
    setUiScale(value)
    localStorage.setItem(UI_SCALE_KEY, value)
    applyUiScale(value)
  }

  useEffect(() => {
    if (core.mode !== 'desktop') return
    void getSystemSnapshot().then(setSystem)
    void getAsrHealth().then(setAsr)
    void getLlmHealth().then(setLlm)
    void getAiProviderStatus().then(setProvider)
    let unlisten: (() => void) | undefined
    void onLlmDownloadProgress(setDownloadProgress).then(value => { unlisten = value })
    return () => unlisten?.()
  }, [core.mode])

  const createDiagnostics = async () => {
    try {
      const report = await exportDiagnostics()
      setDiagnosticMessage(`诊断报告已保存：${report.path}`)
    } catch (error) { setDiagnosticMessage((error as CoreError).message || '无法生成诊断报告。') }
  }
  const installLlm = async () => {
    setDiagnosticMessage('')
    try {
      await downloadLlmModel()
      setLlm(await getLlmHealth())
      setDownloadProgress(undefined)
    } catch (error) {
      setDiagnosticMessage((error as CoreError).message || '模型下载失败。')
      setDownloadProgress(undefined)
    }
  }
  const saveApiKey = async () => {
    if (!apiKey.trim()) return
    setSavingKey(true)
    setDiagnosticMessage('')
    try {
      setProvider(await saveMinimaxApiKey(apiKey))
      setApiKey('')
      setDiagnosticMessage('新密钥已保存到 Windows 凭据管理器，未写入项目文件或浏览器存储。')
    } catch (error) {
      setDiagnosticMessage((error as CoreError).message || '无法安全保存 API 密钥。')
    } finally { setSavingKey(false) }
  }
  const testProvider = async () => {
    setDiagnosticMessage('正在测试 MiniMax-M3…')
    try { setDiagnosticMessage(await testMinimaxConnection()) }
    catch (error) { setDiagnosticMessage((error as CoreError).message || 'MiniMax-M3 连接失败。') }
  }
  const removeApiKey = async () => {
    await deleteMinimaxApiKey()
    setProvider(await getAiProviderStatus())
    setDiagnosticMessage('MiniMax 密钥已从 Windows 凭据管理器删除；AI 文档将使用本地 Qwen 备用。')
  }

  return <div className="page settings-page">
    <div className="page-heading"><span className="section-index">07</span><h1>设置</h1></div>
    <section className="settings-group"><h2>显示与可读性</h2>
      <div className="setting-card"><span className="setting-icon"><Type size={20} /></span><div><strong>界面与字体大小</strong><small>同时放大文字和点击区域，修改后立即生效并在下次启动时保留。</small></div><span>{uiScale === '1' ? '标准' : uiScale === '1.1' ? '较大' : '特大'}</span><select aria-label="界面与字体大小" value={uiScale} onChange={event => changeUiScale(event.target.value as UiScale)}><option value="1">标准 · 100%</option><option value="1.1">较大 · 110%</option><option value="1.25">特大 · 125%</option></select></div>
    </section>
    <section className="settings-group"><h2>AI 文档引擎</h2>
      <div className="setting-card"><span className="setting-icon"><Cloud size={20} /></span><div><strong>主引擎 · MiniMax-M3</strong><small>{provider ? `${provider.endpoint} · ${provider.configured ? '密钥已安全配置' : '等待新密钥'}` : '正在读取安全配置'}</small></div><span className={provider?.configured ? 'privacy-ok' : ''}>{provider?.configured ? '已启用' : '未配置'}</span><button onClick={() => void testProvider()} disabled={!provider?.configured}>测试连接</button></div>
      <div className="setting-card api-key-card"><span className="setting-icon"><KeyRound size={20} /></span><div><strong>{provider?.configured ? '更换 MiniMax API 密钥' : '配置 MiniMax API 密钥'}</strong><small>仅保存在当前 Windows 用户的凭据管理器；请勿再次使用已在聊天中暴露的旧密钥</small><input aria-label="新的 MiniMax API 密钥" type="password" autoComplete="new-password" placeholder="粘贴新生成的密钥" value={apiKey} onChange={event => setApiKey(event.target.value)} /></div><span>不会显示原值</span><button onClick={() => void saveApiKey()} disabled={savingKey || !apiKey.trim()}>{savingKey ? '保存中' : '安全保存'}</button>{provider?.configured && <button className="danger-text-button" aria-label="删除 MiniMax API 密钥" onClick={() => void removeApiKey()}><Trash2 size={16} />删除</button>}</div>
      <div className="setting-card"><span className="setting-icon"><Languages size={20} /></span><div><strong>备用引擎 · 本地 Qwen3-4B</strong><small>{downloadProgress ? `${downloadProgress.stage} · ${Math.round(downloadProgress.progress * 100)}%` : llm ? `${llm.modelName} · ${formatGigabytes(llm.modelSizeBytes)} · ${llm.modelPath}` : '正在检测 llama.cpp 与 GGUF 模型'}</small></div><span className={llm?.available ? 'privacy-ok' : ''}>{llm?.available ? '已就绪' : '尚未就绪'}</span><button disabled={llm?.available || Boolean(downloadProgress)} onClick={() => void installLlm()}>{downloadProgress ? '下载中' : llm?.available ? '已安装' : '镜像下载'}</button></div>
    </section>
    <section className="settings-group"><h2>本地语音与媒体</h2>
      <div className="setting-card"><span className="setting-icon"><Cpu size={20} /></span><div><strong>语音识别引擎</strong><small>{asr ? `faster-whisper ${asr.engineVersion || ''} · Python ${asr.pythonVersion || '未知'}` : core.mode === 'desktop' ? '正在检测 ASR 运行环境' : '桌面端启动后自动检测'}</small></div><span className="recommendation">{asr?.available ? `${asr.cudaDeviceCount} 个 CUDA 设备` : '尚未就绪'}</span><button disabled title={unavailable}>查看详情</button></div>
      <div className="setting-card"><span className="setting-icon"><Download size={20} /></span><div><strong>媒体处理组件</strong><small>识别由 PyAV 解码；FFmpeg / FFprobe 用于可选波形和预处理</small></div><span>{system ? (system.ffmpegAvailable && system.ffprobeAvailable ? '全部就绪' : 'ASR 可独立运行') : '未检测'}</span><button disabled title={unavailable}>管理组件</button></div>
    </section>
    <section className="settings-group"><h2>数据与隐私</h2>
      <div className="setting-card"><span className="setting-icon"><HardDrive size={20} /></span><div><strong>本地数据目录</strong><small>{core.mode === 'desktop' ? core.health.databasePath : '桌面端 AppData / VoiceDesk'}</small></div><span>{formatGigabytes(system?.availableDiskBytes)} 可用</span><button disabled title={unavailable}>打开目录</button></div>
      <div className="setting-card"><span className="setting-icon"><Database size={20} /></span><div><strong>核心与数据库</strong><small>{core.mode === 'desktop' ? `${core.health.projectCount} 个项目 · ${core.health.activeTaskCount} 个活动任务` : '浏览器演示模式未连接数据库'}</small></div><span>{core.mode === 'desktop' ? `v${core.health.version}` : '未连接'}</span><button onClick={() => void createDiagnostics()} disabled={core.mode !== 'desktop'}>生成诊断</button></div>
      <div className="setting-card"><span className="setting-icon"><ShieldCheck size={20} /></span><div><strong>隐私边界</strong><small>录音与语音转文字始终在本机完成；启用 MiniMax 后，仅在你主动生成 AI 文档时发送转写文字。诊断报告不含正文，且不发送遥测。</small></div><span className="privacy-ok">清晰可控</span><button disabled title="完整隐私说明已在首次启动引导中展示">查看说明</button></div>
    </section>
    <div className="prototype-notice"><strong>系统状态</strong><span>{diagnosticMessage || (core.mode === 'desktop' ? (asr?.available && (provider?.configured || llm?.available) ? '语音识别与 AI 已就绪' : asr?.detail || provider?.detail || llm?.detail || '正在检测组件') : '浏览器演示模式')}</span></div>
  </div>
}
