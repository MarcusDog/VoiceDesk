import { AlertTriangle, ArrowUpRight, CheckCircle2, ClipboardCopy, Clock3, FileEdit, LoaderCircle, RefreshCw, Sparkles, Square } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { cancelDocumentGeneration, generateDocument, getAiProviderStatus, getLlmHealth, listAiDocuments, listProjects, onDocumentCreated, onTaskUpdated } from '../core/client'
import type { AiDocumentItem, AiDocumentRecord, AiDocumentType, AiProviderStatus, CoreConnection, LlmHealth, ProjectRecord, TaskRecord } from '../core/types'

type DocumentsPageProps = {
  core: CoreConnection
  activeProjectId?: string
  initialDocumentType?: AiDocumentType
  onOpenProject: (projectId: string) => void
}

const documentTemplates: Record<AiDocumentType, { label: string; sections: { conclusions: string; topics: string; actions: string; risks: string } }> = {
  meeting_notes: { label: '会议纪要', sections: { conclusions: '结论', topics: '关键讨论', actions: '待办事项', risks: '风险与阻塞' } },
  executive_summary: { label: '执行摘要', sections: { conclusions: '核心结论', topics: '关键事实', actions: '下一步', risks: '风险' } },
  action_plan: { label: '行动清单', sections: { conclusions: '目标与决策', topics: '工作事项', actions: '执行清单', risks: '阻塞项' } },
  interview_insights: { label: '访谈洞察', sections: { conclusions: '核心洞察', topics: '主题与观点', actions: '后续研究', risks: '待验证问题' } },
}

const normalizeDocumentType = (value: string): AiDocumentType => value in documentTemplates ? value as AiDocumentType : 'meeting_notes'
const formatUpdated = (value: string) => new Intl.DateTimeFormat('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }).format(new Date(value))

function SourceButton({ item, onOpen }: { item: AiDocumentItem; onOpen: () => void }) {
  if (!item.sourceSequences.length) return null
  return <button onClick={onOpen} title={`转写片段 ${item.sourceSequences.map(value => value + 1).join('、')}`}>S{item.sourceSequences[0] + 1}<ArrowUpRight size={12} /></button>
}

function toMarkdown(document: AiDocumentRecord, projectName: string) {
  const template = documentTemplates[normalizeDocumentType(document.documentType)]
  const list = (items: AiDocumentItem[]) => items.length ? items.map(item => `- ${item.text}`).join('\n') : '- 无'
  const actions = document.content.actionItems.length ? document.content.actionItems.map(item => `- [ ] ${item.text}${item.owner ? ` · ${item.owner}` : ''}${item.due ? ` · ${item.due}` : ''}`).join('\n') : '- 无'
  return `# ${document.title}\n\n${document.content.summary}\n\n## ${template.sections.conclusions}\n${list(document.content.conclusions)}\n\n## ${template.sections.topics}\n${list(document.content.topics)}\n\n## ${template.sections.actions}\n${actions}\n\n## ${template.sections.risks}\n${list(document.content.risks)}\n\n---\n来源：${projectName}`
}

export function DocumentsPage({ core, activeProjectId, initialDocumentType = 'meeting_notes', onOpenProject }: DocumentsPageProps) {
  const [documents, setDocuments] = useState<AiDocumentRecord[]>([])
  const [projects, setProjects] = useState<ProjectRecord[]>([])
  const [selectedId, setSelectedId] = useState<string>()
  const [projectId, setProjectId] = useState(activeProjectId ?? '')
  const [documentType, setDocumentType] = useState<AiDocumentType>(initialDocumentType)
  const [llmHealth, setLlmHealth] = useState<LlmHealth>()
  const [provider, setProvider] = useState<AiProviderStatus>()
  const [runningTask, setRunningTask] = useState<TaskRecord>()
  const [error, setError] = useState('')
  const [copied, setCopied] = useState(false)

  useEffect(() => setDocumentType(initialDocumentType), [initialDocumentType])
  useEffect(() => {
    if (core.mode !== 'desktop') return
    void Promise.all([listAiDocuments(), listProjects(), getLlmHealth(), getAiProviderStatus()]).then(([nextDocuments, nextProjects, health, nextProvider]) => {
      setDocuments(nextDocuments)
      setProjects(nextProjects)
      setLlmHealth(health)
      setProvider(nextProvider)
      setSelectedId(value => value ?? nextDocuments[0]?.id)
      setProjectId(value => value || activeProjectId || nextProjects.find(project => project.status === 'ready')?.id || nextProjects[0]?.id || '')
    }).catch(value => setError(value.message ?? '无法读取本地 AI 文档。'))
    let unlistenDocument: (() => void) | undefined
    let unlistenTask: (() => void) | undefined
    void onDocumentCreated(document => {
      setDocuments(value => [document, ...value.filter(item => item.id !== document.id)])
      setSelectedId(document.id)
    }).then(value => { unlistenDocument = value })
    void onTaskUpdated(task => {
      if (task.kind !== 'llm') return
      setRunningTask(task.status === 'running' || task.status === 'queued' ? task : undefined)
      if (task.status === 'failed') setError(task.errorMessage ?? 'AI 文档生成失败。')
    }).then(value => { unlistenTask = value })
    return () => { unlistenDocument?.(); unlistenTask?.() }
  }, [activeProjectId, core.mode])

  const selected = useMemo(() => documents.find(document => document.id === selectedId) ?? documents[0], [documents, selectedId])
  const projectName = (id: string) => projects.find(project => project.id === id)?.title ?? '未知项目'
  const engineReady = Boolean(provider?.configured || llmHealth?.available)

  const startGeneration = async (type = documentType, sourceProjectId = projectId) => {
    if (!sourceProjectId || runningTask) return
    setError('')
    setDocumentType(type)
    try {
      setRunningTask(await generateDocument(sourceProjectId, type))
    } catch (value) {
      setError((value as { message?: string }).message ?? '无法启动 AI 文档引擎。')
    }
  }

  const copyDocument = async () => {
    if (!selected) return
    await navigator.clipboard.writeText(toMarkdown(selected, projectName(selected.projectId)))
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1600)
  }

  const selectedType = selected ? normalizeDocumentType(selected.documentType) : documentType
  const selectedTemplate = documentTemplates[selectedType]

  return (
    <div className="page documents-page">
      <div className="page-heading documents-heading">
        <div className="heading-cluster"><span className="section-index">06</span><h1>AI 文档</h1></div>
      </div>

      <div className="document-commandbar">
        <select value={projectId} onChange={event => setProjectId(event.target.value)} aria-label="选择来源项目">
          <option value="">选择转写项目</option>
          {projects.map(project => <option key={project.id} value={project.id}>{project.title}</option>)}
        </select>
        <select value={documentType} onChange={event => setDocumentType(event.target.value as AiDocumentType)} aria-label="选择文档模板">
          {Object.entries(documentTemplates).map(([value, template]) => <option key={value} value={value}>{template.label}</option>)}
        </select>
        {runningTask ? <button className="secondary-button document-generate-button" onClick={() => void cancelDocumentGeneration(runningTask.id)}><Square size={15} />取消</button>
          : <button className="primary-button document-generate-button" disabled={!projectId || !engineReady} onClick={() => void startGeneration()}><Sparkles size={17} />生成文档</button>}
      </div>

      {runningTask && <div className="document-progress" role="status"><LoaderCircle size={17} className="spin" /><div><strong>{runningTask.stage}</strong><span>{Math.round(runningTask.progress * 100)}%</span><i><b style={{ width: `${Math.max(3, runningTask.progress * 100)}%` }} /></i></div></div>}
      {error && <div className="inline-error"><AlertTriangle size={16} />{error}</div>}
      {provider && !provider.configured && llmHealth && !llmHealth.available && <div className="prototype-notice"><AlertTriangle size={17} /><span>请先在设置中配置 AI 引擎。</span></div>}

      <div className="document-layout">
        <aside className="document-list">
          <div className="document-list-heading"><strong>文档</strong><span>{documents.length}</span></div>
          {documents.map(document => {
            const template = documentTemplates[normalizeDocumentType(document.documentType)]
            return <button key={document.id} className={selected?.id === document.id ? 'is-active' : ''} onClick={() => setSelectedId(document.id)}><i className="doc-icon doc-icon--green"><FileEdit size={18} /></i><span><strong>{document.title}</strong><small>{template.label} · {formatUpdated(document.updatedAt)}</small></span></button>
          })}
          {!documents.length && <div className="document-empty-list"><FileEdit size={22} /><strong>暂无文档</strong></div>}
        </aside>
        {selected ? <article className="document-preview">
          <header>
            <div className="document-preview-title"><span className="document-type"><CheckCircle2 size={15} />已核对</span><h2>{selected.title}</h2><p><Clock3 size={14} />{selectedTemplate.label} · {projectName(selected.projectId)} · {formatUpdated(selected.updatedAt)}</p></div>
            <div className="document-header-actions"><button className="secondary-button compact" onClick={() => void copyDocument()}><ClipboardCopy size={15} />{copied ? '已复制' : '复制'}</button><button className="secondary-button compact" disabled={!engineReady || Boolean(runningTask)} onClick={() => void startGeneration(selectedType, selected.projectId)}><RefreshCw size={15} />重新生成</button><button className="secondary-button compact" onClick={() => onOpenProject(selected.projectId)}>打开转写</button></div>
          </header>
          <p className="document-summary">{selected.content.summary}</p>
          <div className="doc-grid">
            <section><h3>{selectedTemplate.sections.conclusions}</h3>{selected.content.conclusions.length ? <ol>{selected.content.conclusions.map((item, index) => <li key={index}>{item.text}<SourceButton item={item} onOpen={() => onOpenProject(selected.projectId)} /></li>)}</ol> : <p className="empty-copy">暂无内容</p>}</section>
            <section><h3>{selectedTemplate.sections.topics}</h3>{selected.content.topics.length ? <ul>{selected.content.topics.map((item, index) => <li key={index}>{item.text}<SourceButton item={item} onOpen={() => onOpenProject(selected.projectId)} /></li>)}</ul> : <p className="empty-copy">暂无内容</p>}</section>
            <section className="full"><h3>{selectedTemplate.sections.actions}</h3>{selected.content.actionItems.length ? <div className="tasks">{selected.content.actionItems.map((item, index) => <label key={index}><input type="checkbox" /> {item.text}<span>{item.owner ?? '未指定'}{item.due ? ` · ${item.due}` : ''}</span><SourceButton item={item} onOpen={() => onOpenProject(selected.projectId)} /></label>)}</div> : <p className="empty-copy">暂无内容</p>}</section>
            {selected.content.risks.length > 0 && <section className="full risk-section"><h3>{selectedTemplate.sections.risks}</h3><ul>{selected.content.risks.map((item, index) => <li key={index}>{item.text}<SourceButton item={item} onOpen={() => onOpenProject(selected.projectId)} /></li>)}</ul></section>}
          </div>
        </article> : <article className="document-preview document-empty"><FileEdit size={30} /><h2>选择项目并生成文档</h2><div className="document-empty-templates">{Object.entries(documentTemplates).map(([value, template]) => <button key={value} className={documentType === value ? 'is-active' : ''} onClick={() => setDocumentType(value as AiDocumentType)}>{template.label}</button>)}</div></article>}
      </div>
    </div>
  )
}
