import { BookOpenText, FolderClock, Home, Mic2, Settings, UploadCloud } from 'lucide-react'
import { BrandMark } from './BrandMark'
import type { PageId } from '../types'

type SidebarProps = {
  page: PageId
  collapsed: boolean
  onNavigate: (page: PageId) => void
}

const mainItems = [
  { id: 'home' as const, label: '首页', icon: Home },
  { id: 'import' as const, label: '新建转写', icon: UploadCloud },
  { id: 'library' as const, label: '全部记录', icon: FolderClock },
  { id: 'recorder' as const, label: '录音', icon: Mic2 },
  { id: 'documents' as const, label: 'AI 文档', icon: BookOpenText },
]

export function Sidebar({ page, collapsed, onNavigate }: SidebarProps) {
  return (
    <aside className={`sidebar ${collapsed ? 'sidebar--collapsed' : ''}`}>
      <button className="brand" onClick={() => onNavigate('home')} aria-label="回到首页">
        <BrandMark size="sm" />
        {!collapsed && <span>VoiceDesk</span>}
      </button>

      <nav className="sidebar-nav" aria-label="主导航">
        {mainItems.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            className={`nav-item ${page === id ? 'is-active' : ''}`}
            onClick={() => onNavigate(id)}
            aria-label={label}
            title={collapsed ? label : undefined}
          >
            <Icon size={19} strokeWidth={1.9} />
            {!collapsed && <span>{label}</span>}
          </button>
        ))}
      </nav>

      <div className="sidebar-footer">
        <button className={`nav-item ${page === 'settings' ? 'is-active' : ''}`} onClick={() => onNavigate('settings')} aria-label="设置" title="设置">
          <Settings size={19} strokeWidth={1.9} />
          {!collapsed && <span>设置</span>}
        </button>
      </div>
    </aside>
  )
}
