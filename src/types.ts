import type { LucideIcon } from 'lucide-react'

export type PageId = 'home' | 'import' | 'library' | 'recorder' | 'documents' | 'settings' | 'workspace'

export type Project = {
  id: string
  title: string
  date: string
  duration: string
  kind: '会议' | '访谈' | '灵感' | '课程'
  status: '已完成' | '处理中'
  icon: LucideIcon
}

export type TranscriptLine = {
  time: string
  speaker: string
  initials: string
  color: 'blue' | 'violet' | 'green' | 'amber'
  text: string
}
