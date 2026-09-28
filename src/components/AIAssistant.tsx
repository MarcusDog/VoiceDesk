import { BookOpenText, ClipboardList, FileEdit, FileText, MessagesSquare, UploadCloud } from 'lucide-react'
import type { AiDocumentType } from '../core/types'

type AIAssistantProps = {
  context?: 'home' | 'workspace' | 'document'
  hasProject?: boolean
  onCreateDocument: (type: AiDocumentType) => void
  onOpenDocuments: () => void
  onImport: () => void
}

const templates: Array<{ type: AiDocumentType; label: string; detail: string; icon: typeof FileEdit }> = [
  { type: 'meeting_notes', label: '会议纪要', detail: '摘要 · 决策 · 待办', icon: FileEdit },
  { type: 'executive_summary', label: '执行摘要', detail: '关键结论 · 风险', icon: FileText },
  { type: 'action_plan', label: '行动清单', detail: '负责人 · 截止时间', icon: ClipboardList },
  { type: 'interview_insights', label: '访谈洞察', detail: '主题 · 观点 · 机会', icon: MessagesSquare },
]

export function AIAssistant({ context = 'home', hasProject, onCreateDocument, onOpenDocuments, onImport }: AIAssistantProps) {
  return (
    <aside className="assistant">
      <header className="assistant-header">
        <div><BookOpenText size={18} className="sparkle" /><strong>AI 工具</strong></div>
      </header>
      <div className="assistant-body assistant-body--tools">
        <div className="assistant-section-heading"><strong>文档模板</strong>{context === 'workspace' && <span>{hasProject ? '当前项目' : '未选择项目'}</span>}</div>
        <div className="assistant-actions assistant-template-actions">
          {templates.map(({ type, label, detail, icon: Icon }) => <button key={type} onClick={() => onCreateDocument(type)}><span><Icon size={17} /></span><div><strong>{label}</strong><small>{detail}</small></div></button>)}
        </div>
        <div className="assistant-section-heading assistant-section-heading--secondary"><strong>工作区</strong></div>
        <div className="assistant-utility-actions">
          <button onClick={onOpenDocuments}><BookOpenText size={16} />文档中心</button>
          <button onClick={onImport}><UploadCloud size={16} />导入内容</button>
        </div>
      </div>
    </aside>
  )
}
