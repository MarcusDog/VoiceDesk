import { useEffect, useState } from 'react'
import { AIAssistant } from './components/AIAssistant'
import { CommandPalette } from './components/CommandPalette'
import { Sidebar } from './components/Sidebar'
import { Topbar } from './components/Topbar'
import { Onboarding } from './components/Onboarding'
import { DocumentsPage } from './pages/DocumentsPage'
import { HomePage } from './pages/HomePage'
import { ImportPage } from './pages/ImportPage'
import { LibraryPage } from './pages/LibraryPage'
import { RecorderPage } from './pages/RecorderPage'
import { SettingsPage } from './pages/SettingsPage'
import { WorkspacePage } from './pages/WorkspacePage'
import type { PageId } from './types'
import { connectCore } from './core/client'
import type { AiDocumentType, CoreConnection } from './core/types'

const pageLabels: Record<PageId, string> = {
  home: '首页', import: '新建转写', library: '全部记录', recorder: '录音', documents: 'AI 文档', settings: '设置', workspace: '产品周会讨论',
}

export default function App() {
  const [page, setPage] = useState<PageId>('home')
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const [assistantOpen, setAssistantOpen] = useState(true)
  const [commandOpen, setCommandOpen] = useState(false)
  const [core, setCore] = useState<CoreConnection>({ mode: 'browser', health: null })
  const [activeProjectId, setActiveProjectId] = useState<string>()
  const [documentType, setDocumentType] = useState<AiDocumentType>('meeting_notes')
  const [showOnboarding, setShowOnboarding] = useState(() => localStorage.getItem('voicedesk:onboarding:v2') !== 'complete')

  useEffect(() => {
    void connectCore().then(setCore)
  }, [])

  const openProject = (projectId: string) => {
    setActiveProjectId(projectId)
    setPage('workspace')
  }

  useEffect(() => {
    document.title = `${pageLabels[page]} · VoiceDesk`
  }, [page])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        setCommandOpen(value => !value)
      }
      if ((event.ctrlKey || event.metaKey) && event.key === '\\') {
        event.preventDefault()
        setSidebarCollapsed(value => !value)
      }
      if (event.key === 'Escape') setCommandOpen(false)
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [])

  const renderPage = () => {
    switch (page) {
      case 'home': return <HomePage onNavigate={setPage} onOpenProject={openProject} core={core} />
      case 'import': return <ImportPage onNavigate={setPage} onOpenProject={openProject} core={core} />
      case 'library': return <LibraryPage onNavigate={setPage} onOpenProject={openProject} core={core} />
      case 'recorder': return <RecorderPage onNavigate={setPage} onOpenProject={openProject} core={core} />
      case 'documents': return <DocumentsPage core={core} activeProjectId={activeProjectId} initialDocumentType={documentType} onOpenProject={openProject} />
      case 'settings': return <SettingsPage core={core} />
      case 'workspace': return <WorkspacePage projectId={activeProjectId} core={core} />
    }
  }

  return (
    <main className={`app-shell ${sidebarCollapsed ? 'sidebar-is-collapsed' : ''} ${assistantOpen ? '' : 'assistant-is-closed'}`}>
      <Sidebar page={page} collapsed={sidebarCollapsed} onNavigate={setPage} />
      <div className="app-main">
        <Topbar sidebarCollapsed={sidebarCollapsed} assistantOpen={assistantOpen} onToggleSidebar={() => setSidebarCollapsed(value => !value)} onToggleAssistant={() => setAssistantOpen(value => !value)} onOpenSearch={() => setCommandOpen(true)} core={core} />
        <div className="page-scroll">{renderPage()}</div>
      </div>
      {assistantOpen && <AIAssistant context={page === 'workspace' ? 'workspace' : page === 'documents' ? 'document' : 'home'} hasProject={Boolean(activeProjectId)} onCreateDocument={type => { setDocumentType(type); setPage('documents') }} onOpenDocuments={() => setPage('documents')} onImport={() => setPage('import')} />}
      <CommandPalette open={commandOpen} onClose={() => setCommandOpen(false)} onNavigate={setPage} />
      {showOnboarding && core.mode === 'desktop' && <Onboarding core={core} onComplete={() => setShowOnboarding(false)} onOpenSettings={() => setPage('settings')} />}
    </main>
  )
}
