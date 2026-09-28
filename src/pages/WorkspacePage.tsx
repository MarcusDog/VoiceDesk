import { convertFileSrc } from '@tauri-apps/api/core'
import { Check, Copy, Download, LoaderCircle, MoreHorizontal, Pause, Pencil, Play, ReplaceAll, Search, Square, Volume2, VolumeX, X } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { Waveform } from '../components/Waveform'
import { assignSpeaker, cancelTranscription, exportTranscript, getProjectText, listProjects, listTasks, listTranscriptSegments, onTaskUpdated, onTranscriptSegment, prepareProjectPlayback, replaceTranscriptText, saveProjectText, setSegmentReview, startTranscription, updateTranscriptSegment } from '../core/client'
import type { CoreConnection, ProjectRecord, TaskRecord, TranscriptSegment } from '../core/types'
import { transcript as demoTranscript } from '../data/demo'

type WorkspacePageProps = { projectId?: string; core: CoreConnection }

const formatTime = (milliseconds: number) => {
  const seconds = Math.max(0, Math.floor(milliseconds / 1000))
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor(seconds / 60) % 60
  const rest = seconds % 60
  return hours ? `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(rest).padStart(2, '0')}` : `${String(minutes).padStart(2, '0')}:${String(rest).padStart(2, '0')}`
}

const taskLabel = (task?: TaskRecord) => {
  if (!task) return '正在读取项目'
  if (task.status === 'completed') return '转写已完成'
  if (task.status === 'failed') return task.errorMessage || '转写失败'
  if (task.status === 'cancelled') return '转写已取消'
  return `${task.stage} · ${Math.round(task.progress * 100)}%`
}

export function WorkspacePage({ projectId, core }: WorkspacePageProps) {
  const [tab, setTab] = useState('转写')
  const [playing, setPlaying] = useState(false)
  const [playbackPath, setPlaybackPath] = useState('')
  const [currentTimeMs, setCurrentTimeMs] = useState(0)
  const [playbackRate, setPlaybackRate] = useState(1)
  const [muted, setMuted] = useState(false)
  const audioRef = useRef<HTMLAudioElement>(null)
  const [selected, setSelected] = useState(0)
  const [project, setProject] = useState<ProjectRecord>()
  const [task, setTask] = useState<TaskRecord>()
  const [segments, setSegments] = useState<TranscriptSegment[]>([])
  const [error, setError] = useState('')
  const [exportFormat, setExportFormat] = useState<'md' | 'txt' | 'srt' | 'vtt' | 'json'>('md')
  const [exportMessage, setExportMessage] = useState('')
  const [searchText, setSearchText] = useState('')
  const [replacement, setReplacement] = useState('')
  const [editorText, setEditorText] = useState('')
  const [editorLoadedKind, setEditorLoadedKind] = useState<'notes' | 'draft'>()
  const [editorStatus, setEditorStatus] = useState('')
  const lastSavedText = useRef('')
  const isNative = core.mode === 'desktop' && Boolean(projectId)

  useEffect(() => {
    if (!isNative || !projectId) return
    let disposed = false
    const load = async () => {
      try {
        const [allProjects, allTasks, savedSegments] = await Promise.all([
          listProjects(), listTasks(), listTranscriptSegments(projectId),
        ])
        if (disposed) return
        setProject(allProjects.find(item => item.id === projectId))
        setTask(allTasks.find(item => item.projectId === projectId && item.kind === 'transcription'))
        setSegments(savedSegments)
        const preparedPath = await prepareProjectPlayback(projectId)
        if (!disposed) setPlaybackPath(convertFileSrc(preparedPath))
      } catch (value) {
        if (!disposed) setError(value instanceof Error ? value.message : '无法读取本地项目。')
      }
    }
    void load()
    const unlisteners: Array<() => void> = []
    void onTaskUpdated(updated => {
      if (updated.projectId === projectId && updated.kind === 'transcription') setTask(updated)
    }).then(unlisten => disposed ? unlisten() : unlisteners.push(unlisten))
    void onTranscriptSegment(segment => {
      if (segment.projectId !== projectId) return
      setSegments(current => current.some(item => item.id === segment.id)
        ? current
        : [...current, segment].sort((a, b) => a.sequence - b.sequence))
    }).then(unlisten => disposed ? unlisten() : unlisteners.push(unlisten))
    return () => {
      disposed = true
      unlisteners.forEach(unlisten => unlisten())
    }
  }, [isNative, projectId])

  const editorKind = tab === '笔记' ? 'notes' : tab === '草稿' ? 'draft' : undefined
  useEffect(() => {
    if (!isNative || !projectId || !editorKind) return
    let disposed = false
    setEditorStatus('正在读取…')
    void getProjectText(projectId, editorKind).then(content => {
      if (disposed) return
      lastSavedText.current = content
      setEditorText(content)
      setEditorLoadedKind(editorKind)
      setEditorStatus('已保存')
    }).catch(() => setEditorStatus('读取失败'))
    return () => { disposed = true }
  }, [editorKind, isNative, projectId])

  useEffect(() => {
    if (!projectId || !editorKind || editorLoadedKind !== editorKind || editorText === lastSavedText.current) return
    setEditorStatus('等待自动保存…')
    const timer = window.setTimeout(() => {
      setEditorStatus('正在保存…')
      void saveProjectText(projectId, editorKind, editorText).then(() => {
        lastSavedText.current = editorText
        setEditorStatus('已自动保存')
      }).catch(() => setEditorStatus('保存失败，请继续编辑后重试'))
    }, 700)
    return () => window.clearTimeout(timer)
  }, [editorKind, editorLoadedKind, editorText, projectId])

  const durationMs = useMemo(() => Math.max(project?.durationMs || 0, ...segments.map(segment => segment.endMs), 0), [project?.durationMs, segments])
  const processing = task?.status === 'running' || task?.status === 'queued'
  const visibleSegments = useMemo(() => {
    const reviewFiltered = tab === '审核'
      ? segments.filter(segment => segment.confidence != null && segment.confidence < 0.75 && !segment.reviewStatus)
      : segments
    const query = searchText.trim().toLocaleLowerCase()
    return query ? reviewFiltered.filter(segment => segment.text.toLocaleLowerCase().includes(query)) : reviewFiltered
  }, [searchText, segments, tab])
  const lowConfidenceCount = useMemo(() => segments.filter(segment => segment.confidence != null && segment.confidence < 0.75 && !segment.reviewStatus).length, [segments])
  const chapters = useMemo(() => {
    const bucketSize = durationMs > 60 * 60 * 1000 ? 10 * 60 * 1000 : 5 * 60 * 1000
    const grouped = new Map<number, TranscriptSegment[]>()
    for (const segment of segments) {
      const bucket = Math.floor(segment.startMs / bucketSize)
      grouped.set(bucket, [...(grouped.get(bucket) ?? []), segment])
    }
    return [...grouped.entries()].map(([bucket, items], index) => {
      const firstSentence = items[0]?.text.split(/[。！？!?]/)[0]?.trim() || `第 ${index + 1} 章`
      return {
        startMs: bucket * bucketSize,
        endMs: items.at(-1)?.endMs ?? (bucket + 1) * bucketSize,
        title: firstSentence.length > 28 ? `${firstSentence.slice(0, 28)}…` : firstSentence,
        segmentCount: items.length,
      }
    })
  }, [durationMs, segments])

  const reviewSegment = async (segmentId: string, status: 'accepted' | 'ignored') => {
    try {
      await setSegmentReview(segmentId, status)
      setSegments(current => current.map(item => item.id === segmentId ? { ...item, reviewStatus: status } : item))
    } catch { setError('审核状态未能保存，请重试。') }
  }

  const togglePlayback = async () => {
    const audio = audioRef.current
    if (!audio) return
    if (audio.paused) {
      try { await audio.play(); setPlaying(true) }
      catch { setError('无法播放此媒体格式，请安装 WebView2 媒体组件或改用 WAV/MP3。') }
    } else {
      audio.pause()
      setPlaying(false)
    }
  }
  const seekTo = (milliseconds: number) => {
    const audio = audioRef.current
    if (!audio) return
    audio.currentTime = Math.max(0, milliseconds / 1000)
    setCurrentTimeMs(milliseconds)
    void audio.play().then(() => setPlaying(true)).catch(() => setError('无法从该时间点开始播放。'))
  }
  const seekPercent = (percent: number) => seekTo(percent * durationMs)
  const cycleSpeed = () => {
    const rates = [1, 1.25, 1.5, 2]
    const next = rates[(rates.indexOf(playbackRate) + 1) % rates.length]
    setPlaybackRate(next)
    if (audioRef.current) audioRef.current.playbackRate = next
  }

  const cancel = async () => {
    if (!task) return
    try {
      setTask(await cancelTranscription(task.id))
    } catch (value) {
      setError(typeof value === 'object' && value && 'message' in value ? String(value.message) : '无法取消任务。')
    }
  }

  const retry = async () => {
    if (!task) return
    setError('')
    try {
      setSegments([])
      setTask(await startTranscription(task.id))
    } catch (value) {
      setError(typeof value === 'object' && value && 'message' in value ? String(value.message) : '无法重新开始转写。')
    }
  }

  const runExport = async (format = exportFormat) => {
    if (!projectId || !project) return
    try {
      const path = await exportTranscript(projectId, project.title, format)
      if (path) setExportMessage(`已导出：${path}`)
    } catch (value) {
      setError(typeof value === 'object' && value && 'message' in value ? String(value.message) : '导出失败。')
    }
  }

  const replaceAll = async () => {
    if (!projectId || !searchText) return
    try {
      const count = await replaceTranscriptText(projectId, searchText, replacement)
      setSegments(await listTranscriptSegments(projectId))
      setExportMessage(`已替换 ${count} 个转写片段`)
    } catch (value) {
      setError(typeof value === 'object' && value && 'message' in value ? String(value.message) : '替换失败。')
    }
  }

  return (
    <div className="page workspace-page">
      <header className="workspace-heading">
        <div className="workspace-title"><span className="section-index">05</span><div><span className="overline">{isNative ? '本地项目' : '产品项目 · 演示内容'}</span><h1>{project?.title || (isNative ? '正在加载…' : '产品周会讨论')} <button aria-label="重命名" disabled={isNative} title={isNative ? '项目重命名将在资料管理功能完成后开放' : undefined}><Pencil size={16} /></button></h1><p>{formatTime(durationMs || 4328000)} · {isNative ? taskLabel(task) : '4 位发言人 · 中文（普通话）'}</p></div></div>
        <div className="heading-actions">
          {processing ? <span className="processing-badge"><LoaderCircle size={15} /> 本地识别中 {Math.round((task?.progress || 0) * 100)}%</span> : <span className="saved"><Check size={15} /> 已保存</span>}
          {processing && <button className="secondary-button compact" onClick={() => void cancel()}><Square size={14} /> 取消</button>}
          {(task?.status === 'failed' || task?.status === 'paused') && <button className="secondary-button compact" onClick={() => void retry()}>重新转写</button>}
          {isNative && segments.length > 0 && <><select className="export-select" value={exportFormat} onChange={event => setExportFormat(event.target.value as typeof exportFormat)}><option value="md">Markdown</option><option value="txt">TXT</option><option value="srt">SRT</option><option value="vtt">VTT</option><option value="json">JSON</option></select><button className="secondary-button compact" onClick={() => void runExport()}><Download size={14} /> 导出</button></>}
          <details className="project-menu">
            <summary className="icon-button" aria-label="更多项目操作" title="更多项目操作"><MoreHorizontal size={18} /></summary>
            <div className="project-menu-popover">
              <button onClick={event => { setTab('章节'); (event.currentTarget.closest('details') as HTMLDetailsElement | null)?.removeAttribute('open') }}><Play size={14} />查看章节</button>
              <button disabled={!segments.length} onClick={event => { void runExport('md'); (event.currentTarget.closest('details') as HTMLDetailsElement | null)?.removeAttribute('open') }}><Download size={14} />导出 Markdown</button>
              <button disabled={!segments.length} onClick={event => { void runExport('txt'); (event.currentTarget.closest('details') as HTMLDetailsElement | null)?.removeAttribute('open') }}><Download size={14} />导出纯文本</button>
            </div>
          </details>
        </div>
      </header>

      {error && <div className="form-error" role="alert">{error}</div>}
      {task?.status === 'failed' && <div className="transcription-error" role="alert"><strong>转写失败</strong><span>{task.errorMessage}</span><small>{task.errorCode}</small></div>}
      <section className="workspace-card">
        <div className="workspace-tabs">
          {['转写', '审核', '章节', '笔记', '草稿'].map(item => <button key={item} className={tab === item ? 'is-active' : ''} disabled={!isNative && ['章节', '笔记', '草稿'].includes(item)} onClick={() => setTab(item)}>{item}{item === '审核' && isNative ? ` ${lowConfidenceCount}` : ''}</button>)}
        </div>
        {!editorKind && <div className="audio-player">
          {isNative && playbackPath && <audio ref={audioRef} src={playbackPath} preload="metadata" muted={muted} onTimeUpdate={event => setCurrentTimeMs(event.currentTarget.currentTime * 1000)} onPlay={() => setPlaying(true)} onPause={() => setPlaying(false)} onEnded={() => setPlaying(false)} />}
          <button className="play-button" disabled={isNative && !playbackPath} onClick={() => isNative ? void togglePlayback() : setPlaying(!playing)} aria-label={playing ? '暂停' : '播放'}>{playing ? <Pause size={18} fill="currentColor" /> : <Play size={18} fill="currentColor" />}</button>
          <span className="play-time">{formatTime(isNative ? currentTimeMs : (segments[selected]?.startMs ?? 0))} <i>/ {formatTime(durationMs || 4328000)}</i></span>
          <Waveform dense active={processing ? Math.max(4, Math.round((task?.progress || 0) * 100)) : isNative ? (durationMs ? currentTimeMs / durationMs * 100 : 0) : playing ? 62 : 35} onSeek={isNative && playbackPath ? seekPercent : undefined} />
          <button className="speed-button" disabled={isNative && !playbackPath} onClick={isNative ? cycleSpeed : undefined}>{playbackRate}×</button>
          <button className="icon-button" disabled={isNative && !playbackPath} onClick={() => setMuted(value => !value)} aria-label={muted ? '取消静音' : '静音'}>{muted ? <VolumeX size={17} /> : <Volume2 size={17} />}</button>
        </div>}
        {!editorKind && tab !== '章节' && isNative && segments.length > 0 && <div className="transcript-searchbar">
          <label><Search size={14} /><input value={searchText} onChange={event => setSearchText(event.target.value)} placeholder="在转写中查找" aria-label="查找转写文字" /></label>
          <span>{searchText ? `${visibleSegments.length} 个片段` : `${segments.length} 个片段`}</span>
          <label><ReplaceAll size={14} /><input value={replacement} onChange={event => setReplacement(event.target.value)} placeholder="替换为" aria-label="替换文字" /></label>
          <button className="secondary-button compact" disabled={!searchText} onClick={() => void replaceAll()}>全部替换</button>
        </div>}
        {tab === '章节' ? <div className="chapter-list">
          <header><div><strong>自动章节</strong><span>根据录音长度和时间轴在本机自动分段，标题取自每段首句。</span></div><small>{chapters.length} 章</small></header>
          {chapters.map((chapter, index) => <button key={chapter.startMs} onClick={() => seekTo(chapter.startMs)} disabled={!playbackPath}><span>{String(index + 1).padStart(2, '0')}</span><div><strong>{chapter.title}</strong><small>{formatTime(chapter.startMs)}–{formatTime(chapter.endMs)} · {chapter.segmentCount} 个片段</small></div><Play size={15} /></button>)}
          {!chapters.length && <div className="transcript-empty"><strong>暂无章节</strong><span>转写完成后会自动生成章节导航。</span></div>}
        </div> : editorKind ? <div className="project-text-editor">
          <header><div><strong>{editorKind === 'notes' ? '个人笔记' : '个人草稿'}</strong><span>{editorKind === 'notes' ? '记录自己的想法，不会发送给 AI。' : '整理可继续编辑的成稿内容。'}</span></div><small>{editorStatus}</small></header>
          <textarea aria-label={editorKind === 'notes' ? '个人笔记编辑器' : '个人草稿编辑器'} value={editorText} onChange={event => setEditorText(event.target.value)} onBlur={() => { if (projectId && editorText !== lastSavedText.current) void saveProjectText(projectId, editorKind, editorText).then(() => { lastSavedText.current = editorText; setEditorStatus('已自动保存') }).catch(() => setEditorStatus('保存失败，请重试')) }} placeholder={editorKind === 'notes' ? '写下补充信息、灵感或核对结果…' : '在这里整理最终文稿…'} />
          <footer>{editorText.length.toLocaleString('zh-CN')} 字符 · 本机 SQLite 自动保存</footer>
        </div> : <div className="transcript-list" aria-live="polite">
          {isNative && tab === '审核' && segments.length > 0 && !visibleSegments.length && <div className="transcript-empty"><Check size={26} /><strong>没有低置信度片段</strong><span>当前所有片段的识别置信度都高于审核阈值。</span></div>}
          {isNative && !segments.length && <div className="transcript-empty">
            {processing ? <><LoaderCircle size={26} /><strong>{task?.stage || '正在启动识别引擎'}</strong><span>{task?.stage?.includes('下载') ? '本机尚无完整模型，正在通过镜像下载；完成后会自动复用。' : '正在使用本机已有识别模型，不会重复下载。'}</span><progress value={task?.progress || 0} max={1} /></> : <><strong>暂无转写内容</strong><span>{taskLabel(task)}</span></>}
          </div>}
          {isNative ? visibleSegments.map((line) => {
            const index = segments.findIndex(segment => segment.id === line.id)
            return (
            <article key={line.id} className={`transcript-row ${selected === index ? 'is-selected' : ''} ${tab === '审核' ? 'is-reviewing' : ''}`} onClick={() => setSelected(index)}>
              <button className="timestamp" disabled={!playbackPath} title={playbackPath ? '从此处播放' : '正在准备媒体'} onClick={(event) => { event.stopPropagation(); seekTo(line.startMs) }}>{formatTime(line.startMs)}</button>
              <span className="speaker-avatar speaker-avatar--blue">声</span>
              <strong contentEditable suppressContentEditableWarning title="编辑后按 Enter 或点击别处，应用到同一说话人" onKeyDown={event => { if (event.key === 'Enter') { event.preventDefault(); event.currentTarget.blur() } }} onBlur={event => {
                const name = event.currentTarget.textContent?.trim() || line.speaker || '说话人'
                if (name !== (line.speaker || '说话人')) void assignSpeaker(line.id, name, true).then(async () => setSegments(await listTranscriptSegments(projectId!))).catch(() => setError('说话人名称未能保存。'))
              }}>{line.speaker || '说话人'}</strong>
              <p contentEditable suppressContentEditableWarning title={line.confidence == null ? undefined : `识别置信度 ${Math.round(line.confidence * 100)}%`} onBlur={event => {
                const text = event.currentTarget.textContent?.trim() || line.text
                if (text !== line.text) {
                  setSegments(current => current.map(item => item.id === line.id ? { ...item, text } : item))
                  void updateTranscriptSegment(line.id, text).catch(() => setError('修改未能保存，请重试。'))
                }
              }}>{line.text}</p>
              <div className="row-actions">{tab === '审核' && <><button aria-label="接受此片段" title="接受" onClick={event => { event.stopPropagation(); void reviewSegment(line.id, 'accepted') }}><Check size={15} /></button><button aria-label="忽略此片段" title="忽略" onClick={event => { event.stopPropagation(); void reviewSegment(line.id, 'ignored') }}><X size={15} /></button></>}<button aria-label="复制" title="复制文字" onClick={event => { event.stopPropagation(); void navigator.clipboard.writeText(line.text) }}><Copy size={15} /></button><details onClick={event => event.stopPropagation()}><summary aria-label="更多片段操作" title="更多片段操作"><MoreHorizontal size={16} /></summary><div className="segment-menu-popover"><button disabled={!playbackPath} onClick={event => { seekTo(line.startMs); (event.currentTarget.closest('details') as HTMLDetailsElement | null)?.removeAttribute('open') }}><Play size={14} />从此处播放</button><button onClick={event => { void navigator.clipboard.writeText(line.text); (event.currentTarget.closest('details') as HTMLDetailsElement | null)?.removeAttribute('open') }}><Copy size={14} />复制文字</button>{line.confidence != null && line.confidence < 0.75 && !line.reviewStatus && <button onClick={event => { void reviewSegment(line.id, 'accepted'); (event.currentTarget.closest('details') as HTMLDetailsElement | null)?.removeAttribute('open') }}><Check size={14} />标记已审核</button>}</div></details></div>
            </article>
          )}) : demoTranscript.map((line, index) => (
            <article key={line.time} className={`transcript-row ${selected === index ? 'is-selected' : ''}`} onClick={() => setSelected(index)}>
              <button className="timestamp" onClick={(event) => { event.stopPropagation(); setPlaying(true) }}>{line.time}</button>
              <span className={`speaker-avatar speaker-avatar--${line.color}`}>{line.initials}</span><strong>{line.speaker}</strong>
              <p contentEditable suppressContentEditableWarning>{line.text}</p>
              <div className="row-actions"><button aria-label="复制" onClick={event => { event.stopPropagation(); void navigator.clipboard.writeText(line.text) }}><Copy size={15} /></button><details onClick={event => event.stopPropagation()}><summary aria-label="更多片段操作"><MoreHorizontal size={16} /></summary><div className="segment-menu-popover"><button onClick={event => { void navigator.clipboard.writeText(line.text); (event.currentTarget.closest('details') as HTMLDetailsElement | null)?.removeAttribute('open') }}><Copy size={14} />复制文字</button></div></details></div>
            </article>
          ))}
        </div>}
      </section>
      {exportMessage && <div className="export-message"><Check size={14} /> {exportMessage}</div>}
      <div className="source-trace"><Check size={17} /><strong>本地处理</strong></div>
    </div>
  )
}
