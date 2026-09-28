import { Menu, PanelRightClose, PanelRightOpen, Search } from 'lucide-react'
import type { CoreConnection } from '../core/types'

type TopbarProps = {
  sidebarCollapsed: boolean
  assistantOpen: boolean
  onToggleSidebar: () => void
  onToggleAssistant: () => void
  onOpenSearch: () => void
  core: CoreConnection
}

export function Topbar({ sidebarCollapsed, assistantOpen, onToggleSidebar, onToggleAssistant, onOpenSearch, core }: TopbarProps) {
  const coreLabel = core.mode === 'desktop'
    ? `本地核心 v${core.health.version}`
    : core.mode === 'error' ? '本地核心异常' : '浏览器演示'
  return (
    <header className="topbar">
      <button className="icon-button" onClick={onToggleSidebar} aria-label={sidebarCollapsed ? '展开侧边栏' : '收起侧边栏'} title="收起/展开侧边栏 Ctrl + \">
        <Menu size={19} />
      </button>
      <button className="global-search" onClick={onOpenSearch}>
        <Search size={17} />
        <span>搜索记录、文档或命令…</span>
        <kbd>Ctrl K</kbd>
      </button>
      <div className="topbar-spacer" />
      <span className={`local-indicator ${core.mode === 'error' ? 'has-error' : ''}`} title={core.mode === 'error' ? core.error.message : undefined}><i /> {coreLabel}</span>
      <button className="icon-button" onClick={onToggleAssistant} aria-label={assistantOpen ? '收起 AI 助手' : '展开 AI 助手'}>
        {assistantOpen ? <PanelRightClose size={19} /> : <PanelRightOpen size={19} />}
      </button>
    </header>
  )
}
