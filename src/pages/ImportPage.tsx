import { Check, FileAudio2, FileVideo2, FolderOpen, Sparkles, UploadCloud, Users, Zap } from 'lucide-react'
import { useRef, useState } from 'react'
import type { PageId } from '../types'
import { createProject, enqueueTask, pickMediaFiles, probeMedia, registerMedia, startTranscription as runTranscription } from '../core/client'
import type { CoreConnection, CoreError, MediaProbe } from '../core/types'

type ImportedFile = { name: string; size: string; kind: 'audio' | 'video'; path?: string; probe?: MediaProbe; state?: 'probing' | 'ready' | 'error'; error?: string }
type ImportPageProps = { onNavigate: (page: PageId) => void; onOpenProject: (projectId: string) => void; core: CoreConnection }

const demoFiles: ImportedFile[] = [
  { name: '产品周会讨论.mp3', size: '28.6 MB · 00:48:32', kind: 'audio' },
  { name: '用户访谈录像.mp4', size: '152.3 MB · 01:26:15', kind: 'video' },
]

const fileNameFromPath = (path: string) => path.split(/[\\/]/).pop() || '未命名媒体'
const formatBytes = (bytes: number) => `${Math.max(0.1, bytes / 1024 / 1024).toFixed(1)} MB`
const formatDuration = (milliseconds: number) => {
  const seconds = Math.floor(milliseconds / 1000)
  return [Math.floor(seconds / 3600), Math.floor(seconds / 60) % 60, seconds % 60].map(value => String(value).padStart(2, '0')).join(':')
}

export function ImportPage({ onNavigate, onOpenProject, core }: ImportPageProps) {
  const [files, setFiles] = useState<ImportedFile[]>(demoFiles)
  const [mode, setMode] = useState('自动推荐')
  const [submitting, setSubmitting] = useState(false)
  const [submitError, setSubmitError] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  const addFiles = (list: FileList | null) => {
    if (!list) return
    setFiles(current => [...current, ...Array.from(list).map(file => ({
      name: file.name,
      size: `${Math.max(0.1, file.size / 1024 / 1024).toFixed(1)} MB · 待分析`,
      kind: /video/i.test(file.type) ? 'video' as const : 'audio' as const,
      state: 'ready' as const,
    }))])
  }

  const chooseFiles = async () => {
    const paths = await pickMediaFiles()
    if (paths === null) {
      inputRef.current?.click()
      return
    }
    if (!paths.length) return
    const pendingFiles: ImportedFile[] = paths.map(path => ({
      name: fileNameFromPath(path),
      size: '正在读取媒体信息…',
      kind: /\.(mp4|mov|mkv)$/i.test(path) ? 'video' : 'audio',
      path,
      state: 'probing',
    }))
    setFiles(current => [...current, ...pendingFiles])
    for (const path of paths) {
      try {
        const probe = await probeMedia(path)
        setFiles(current => current.map(file => file.path === path ? { ...file, name: probe.displayName, kind: probe.hasVideo ? 'video' : 'audio', size: `${formatBytes(probe.sizeBytes)} · ${formatDuration(probe.durationMs)}`, probe, state: 'ready' } : file))
      } catch (error) {
        const coreError = error as CoreError
        setFiles(current => current.map(file => file.path === path ? { ...file, state: 'error', error: coreError.message, size: '读取失败' } : file))
      }
    }
  }

  const startTranscription = async () => {
    if (!files.length || submitting) return
    if (core.mode !== 'desktop') {
      onNavigate('workspace')
      return
    }
    const nativeFiles = files.filter(file => file.path && file.probe && file.state === 'ready')
    if (!nativeFiles.length) {
      setSubmitError('请先选择并成功分析至少一个本地媒体文件。')
      return
    }
    setSubmitting(true)
    setSubmitError('')
    try {
      const project = await createProject(nativeFiles[0].name.replace(/\.[^.]+$/, ''))
      for (const file of nativeFiles) {
        await registerMedia(project.id, file.probe!)
      }
      const model = mode === '快速' ? 'base' : mode === '自动推荐' ? 'small' : 'medium'
      const task = await enqueueTask('transcription', project.id, {
        mediaPaths: nativeFiles.map(file => file.path),
        mode,
        model,
        language: 'zh',
        device: 'auto',
        computeType: 'auto',
        beamSize: mode === '快速' ? 3 : 5,
      })
      await runTranscription(task.id)
      onOpenProject(project.id)
    } catch (error) {
      setSubmitError((error as CoreError).message || '无法创建转写任务。')
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="page import-page">
      <div className="page-heading">
        <span className="section-index">02</span>
        <h1>新建转写</h1>
      </div>

      <section
        className="drop-zone"
        onDragOver={(event) => event.preventDefault()}
        onDrop={(event) => { event.preventDefault(); addFiles(event.dataTransfer.files) }}
      >
        <span className="drop-icon"><UploadCloud size={32} /></span>
        <h2>拖拽音视频文件到这里</h2>
        <p>支持 MP3、WAV、M4A、AAC、FLAC、MP4、MOV，单个文件建议不超过 2 GB</p>
        <button className="secondary-button" onClick={() => void chooseFiles()}><FolderOpen size={17} /> 选择文件</button>
        <input ref={inputRef} type="file" multiple accept="audio/*,video/*" hidden onChange={(event) => addFiles(event.target.files)} />
      </section>

      <section className="imported-files">
        <div className="subsection-heading"><h2>已添加 {files.length} 个文件</h2><button onClick={() => setFiles([])}>清空列表</button></div>
        <div className="file-list">
          {files.map((file, index) => {
            const Icon = file.kind === 'video' ? FileVideo2 : FileAudio2
            return <div className={`file-row ${file.state === 'error' ? 'has-error' : ''}`} key={`${file.name}-${index}`}><span><Icon size={19} /></span><div><strong>{file.name}</strong><small>{file.error || file.size}</small></div><em>{file.state === 'probing' ? '分析中…' : file.state === 'error' ? '不可用' : <><Check size={15} /> 已添加</>}</em><button aria-label={`移除 ${file.name}`} onClick={() => setFiles(files.filter((_, fileIndex) => fileIndex !== index))}>×</button></div>
          })}
          {!files.length && <div className="inline-empty">还没有文件，拖入音频即可开始。</div>}
        </div>
      </section>

      <section className="mode-section">
        <h2>转写模式</h2>
        <div className="mode-grid">
          {[
            { name: '自动推荐', detail: '根据内容智能平衡效果与速度', icon: Sparkles },
            { name: '快速', detail: '适合普通录音和快速浏览', icon: Zap },
            { name: '高精度', detail: '更高识别准确率，耗时更长', icon: Check },
            { name: '多人会议', detail: '识别不同发言人并自动分段', icon: Users },
          ].map(({ name, detail, icon: Icon }) => (
            <button className={mode === name ? 'is-selected' : ''} key={name} onClick={() => setMode(name)}><Icon size={20} /><strong>{name}</strong><small>{detail}</small></button>
          ))}
        </div>
      </section>

      <div className="form-row">
        <label>转写语言<select defaultValue="中文（普通话）"><option>中文（普通话）</option><option>自动检测</option><option>English</option></select></label>
        <label>专业词汇<select defaultValue="默认词库"><option>默认词库</option><option>产品研发词库</option><option>新建词库…</option></select></label>
        <button className="primary-button" disabled={!files.length || submitting || files.some(file => file.state === 'probing')} onClick={() => void startTranscription()}>{submitting ? '正在创建任务…' : `开始转写（${files.length} 个文件）`}</button>
      </div>
      {submitError && <div className="form-error" role="alert">{submitError}</div>}
    </div>
  )
}
