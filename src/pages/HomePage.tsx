import { ArrowRight, FileAudio2, Mic2, MoreHorizontal, UploadCloud } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { listAiDocuments, listProjects } from '../core/client'
import type { CoreConnection, ProjectRecord } from '../core/types'
import { projects as demoProjects } from '../data/demo'
import type { PageId } from '../types'

type HomePageProps = {
  onNavigate: (page: PageId) => void
  onOpenProject: (projectId: string) => void
  core: CoreConnection
}

const formatDuration = (milliseconds: number) => {
  const minutes = Math.max(0, Math.round(milliseconds / 60_000))
  return minutes >= 60 ? `${Math.floor(minutes / 60)} 小时 ${minutes % 60} 分钟` : `${minutes} 分钟`
}

export function HomePage({ onNavigate, onOpenProject, core }: HomePageProps) {
  const [nativeProjects, setNativeProjects] = useState<ProjectRecord[]>([])
  const [documentCount, setDocumentCount] = useState(0)
  useEffect(() => {
    if (core.mode !== 'desktop') return
    void Promise.all([listProjects(), listAiDocuments()]).then(([projects, documents]) => {
      setNativeProjects(projects.slice(0, 5))
      setDocumentCount(documents.length)
    })
  }, [core.mode])
  const totalDuration = useMemo(() => nativeProjects.reduce((sum, project) => sum + project.durationMs, 0), [nativeProjects])

  return (
    <div className="page home-page">
      <section className="home-hero">
        <div><h1>今天想整理什么？</h1></div>
        <div className="hero-visual" aria-hidden="true"><i /><i /><i /><span /></div>
      </section>
      <section className="quick-actions">
        <button className="action-card action-card--blue" onClick={() => onNavigate('import')}><span className="action-icon"><UploadCloud size={25} /></span><span><strong>导入音视频</strong><small>支持多种格式，智能识别并生成文字记录</small></span><i className="round-arrow"><ArrowRight size={18} /></i></button>
        <button className="action-card action-card--violet" onClick={() => onNavigate('recorder')}><span className="action-icon"><Mic2 size={25} /></span><span><strong>开始录音</strong><small>实时采集，本地保存，录制完成自动转写</small></span><i className="round-arrow"><ArrowRight size={18} /></i></button>
      </section>
      <section className="content-section">
        <header className="section-header"><div><span className="section-index">01</span><h2>最近项目</h2></div><button className="text-button" onClick={() => onNavigate('library')}>查看全部 <ArrowRight size={15} /></button></header>
        <div className="project-list">
          {core.mode === 'desktop' ? nativeProjects.map(project => <button className="project-row" key={project.id} onClick={() => onOpenProject(project.id)}><span className="project-icon"><FileAudio2 size={20} /></span><span className="project-title"><strong>{project.title}</strong><small>{new Date(project.updatedAt).toLocaleString('zh-CN')} · {formatDuration(project.durationMs)}</small></span><span className={`status-pill ${project.status === 'processing' ? 'is-processing' : ''}`}>{project.status === 'processing' ? '处理中' : project.status === 'failed' ? '失败' : '已保存'}</span><span className="icon-button"><MoreHorizontal size={18} /></span></button>) : demoProjects.map(({ id, title, date, duration, kind, status, icon: Icon }) => <button className="project-row" key={id} onClick={() => onNavigate('workspace')}><span className="project-icon"><Icon size={20} /></span><span className="project-title"><strong>{title}</strong><small>{date} · {duration}</small></span>{status === '处理中' ? <span className="status-pill is-processing">处理中</span> : <span className={`tag tag--${kind}`}>{kind}</span>}<span className="icon-button"><MoreHorizontal size={18} /></span></button>)}
          {core.mode === 'desktop' && !nativeProjects.length && <div className="large-empty"><FileAudio2 size={28} /><strong>从第一段声音开始</strong><span>导入文件或开始录音，项目会安全保存在本机。</span></div>}
        </div>
      </section>
      <section className="insight-strip"><FileAudio2 size={19} /><p><strong>{core.mode === 'desktop' ? `已整理 ${formatDuration(totalDuration)}音频` : '本周已整理 4 小时 36 分钟音频'}</strong><span>{core.mode === 'desktop' ? `已生成 ${documentCount} 份可追溯 AI 文档` : '比上周多沉淀了 12 条可追溯结论'}</span></p><button onClick={() => onNavigate('documents')}>查看 AI 文档</button></section>
    </div>
  )
}
