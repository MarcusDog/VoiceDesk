import { BookOpenText, FileAudio, Home, Mic2, Search, Settings, UploadCloud, X } from 'lucide-react'
import { useMemo, useState } from 'react'
import type { PageId } from '../types'

type CommandPaletteProps = {
  open: boolean
  onClose: () => void
  onNavigate: (page: PageId) => void
}

const commands = [
  { label: '回到首页', hint: '导航', page: 'home' as const, icon: Home },
  { label: '导入音视频', hint: '新建转写', page: 'import' as const, icon: UploadCloud },
  { label: '开始录音', hint: '实时采集', page: 'recorder' as const, icon: Mic2 },
  { label: '查看全部记录', hint: '资料库', page: 'library' as const, icon: FileAudio },
  { label: '打开 AI 文档', hint: '内容资产', page: 'documents' as const, icon: BookOpenText },
  { label: '打开设置', hint: '偏好与模型', page: 'settings' as const, icon: Settings },
]

export function CommandPalette({ open, onClose, onNavigate }: CommandPaletteProps) {
  const [query, setQuery] = useState('')
  const visible = useMemo(() => commands.filter(item => `${item.label}${item.hint}`.toLowerCase().includes(query.toLowerCase())), [query])
  if (!open) return null

  return (
    <div className="modal-backdrop" onMouseDown={onClose} role="presentation">
      <section className="command-palette" role="dialog" aria-modal="true" aria-label="搜索与命令" onMouseDown={(event) => event.stopPropagation()}>
        <div className="command-input">
          <Search size={19} />
          <input autoFocus value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索记录、文档、功能或输入命令…" />
          <button className="icon-button" onClick={onClose} aria-label="关闭"><X size={17} /></button>
        </div>
        <div className="command-section-label">快捷操作</div>
        <div className="command-results">
          {visible.map(({ label, hint, page, icon: Icon }) => (
            <button key={label} onClick={() => { onNavigate(page); onClose() }}>
              <span className="command-icon"><Icon size={18} /></span>
              <span><strong>{label}</strong><small>{hint}</small></span>
              <kbd>↵</kbd>
            </button>
          ))}
          {!visible.length && <div className="empty-command">没有找到匹配内容</div>}
        </div>
        <footer><span>↑↓ 选择</span><span>Enter 打开</span><span>Esc 关闭</span></footer>
      </section>
    </div>
  )
}
