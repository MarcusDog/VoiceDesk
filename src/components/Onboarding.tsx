import { Check, ChevronLeft, ChevronRight, Cpu, HardDrive, Mic2, ShieldCheck, Sparkles } from 'lucide-react'
import { useEffect, useState } from 'react'
import { getAiProviderStatus, getAsrHealth, getLlmHealth, getSystemSnapshot } from '../core/client'
import type { AiProviderStatus, AsrHealth, CoreConnection, LlmHealth, SystemSnapshot } from '../core/types'

type OnboardingProps = {
  core: CoreConnection
  onComplete: () => void
  onOpenSettings: () => void
}

const gb = (value?: number) => value ? `${(value / 1024 ** 3).toFixed(1)} GB` : '检测中…'

export function Onboarding({ core, onComplete, onOpenSettings }: OnboardingProps) {
  const [step, setStep] = useState(0)
  const [system, setSystem] = useState<SystemSnapshot>()
  const [asr, setAsr] = useState<AsrHealth>()
  const [llm, setLlm] = useState<LlmHealth>()
  const [provider, setProvider] = useState<AiProviderStatus>()

  useEffect(() => {
    if (step !== 2 || core.mode !== 'desktop') return
    void Promise.all([getSystemSnapshot(), getAsrHealth(), getLlmHealth(), getAiProviderStatus()]).then(([nextSystem, nextAsr, nextLlm, nextProvider]) => {
      setSystem(nextSystem); setAsr(nextAsr); setLlm(nextLlm); setProvider(nextProvider)
    })
  }, [core.mode, step])

  const finish = () => {
    localStorage.setItem('voicedesk:onboarding:v2', 'complete')
    onComplete()
  }

  return <div className="onboarding-backdrop" role="dialog" aria-modal="true" aria-labelledby="onboarding-title">
    <section className="onboarding-card">
      <div className="onboarding-progress" aria-label={`首次设置第 ${step + 1} 步，共 3 步`}><i className={step >= 0 ? 'is-active' : ''} /><i className={step >= 1 ? 'is-active' : ''} /><i className={step >= 2 ? 'is-active' : ''} /></div>
      {step === 0 && <div className="onboarding-content"><span className="onboarding-mark"><Mic2 size={29} /></span><p className="eyebrow">欢迎使用 VoiceDesk</p><h1 id="onboarding-title">把声音变成可以继续使用的知识。</h1><p>无需账号。录音、转写和 AI 文档都可以在这台电脑上完成。</p><div className="onboarding-points"><span><Check size={15} />真实语音转文字</span><span><Check size={15} />可追溯 AI 会议纪要</span><span><Check size={15} />本地 SQLite 自动保存</span></div></div>}
      {step === 1 && <div className="onboarding-content"><span className="onboarding-mark is-green"><ShieldCheck size={29} /></span><p className="eyebrow">隐私说明</p><h1 id="onboarding-title">语音本地处理，云端调用由你决定。</h1><p>VoiceDesk 不要求账号。音频和语音识别留在本机；启用 MiniMax 后，仅在你主动生成 AI 文档时发送转写文字。</p><div className="privacy-grid"><div><strong>音频与录音</strong><span>保留在原文件位置或应用数据目录，不上传</span></div><div><strong>转写与文档</strong><span>写入本机 SQLite，不进入诊断报告</span></div><div><strong>MiniMax-M3</strong><span>只接收主动生成文档所需的转写文字</span></div><div><strong>密钥与统计</strong><span>密钥进入 Windows 凭据库；当前版本不发送遥测</span></div></div></div>}
      {step === 2 && <div className="onboarding-content"><span className="onboarding-mark is-violet"><Cpu size={29} /></span><p className="eyebrow">本机准备情况</p><h1 id="onboarding-title">准备你的个人语音工作区。</h1><div className="hardware-list"><div><Cpu size={18} /><span><strong>{system?.cpuName ?? '正在检测处理器'}</strong><small>{system ? `${system.cpuCount} 线程 · ${gb(system.totalMemoryBytes)} 内存` : '请稍候…'}</small></span><Check size={17} /></div><div><Mic2 size={18} /><span><strong>本地语音识别</strong><small>{asr?.available ? `faster-whisper 已就绪 · ${asr.cudaDeviceCount} 个 CUDA 设备` : asr?.detail ?? '正在检测…'}</small></span><i className={asr?.available ? 'status-ok' : 'status-wait'}>{asr?.available ? '就绪' : '待准备'}</i></div><div><Sparkles size={18} /><span><strong>MiniMax-M3 主引擎</strong><small>{provider?.configured ? '新密钥已安全配置' : '请到设置中保存新密钥'}</small></span><i className={provider?.configured ? 'status-ok' : 'status-wait'}>{provider?.configured ? '就绪' : '待配置'}</i></div><div><HardDrive size={18} /><span><strong>本地 Qwen 备用</strong><small>{llm?.available ? `${llm.modelName} · ${gb(llm.modelSizeBytes)}` : '约 2.5 GB，可使用镜像下载'}</small></span><i className={llm?.available ? 'status-ok' : 'status-wait'}>{llm?.available ? '就绪' : '未下载'}</i></div></div></div>}
      <footer>{step > 0 ? <button className="secondary-button" onClick={() => setStep(value => value - 1)}><ChevronLeft size={16} />返回</button> : <span />}{step < 2 ? <button className="primary-button" onClick={() => setStep(value => value + 1)}>继续<ChevronRight size={16} /></button> : <div className="onboarding-final-actions">{!provider?.configured && <button className="secondary-button" onClick={() => { finish(); onOpenSettings() }}>配置 MiniMax</button>}<button className="primary-button" onClick={finish}>进入工作区</button></div>}</footer>
    </section>
  </div>
}
