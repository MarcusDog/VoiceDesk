import { BookOpen, Lightbulb, Mic2, Users } from 'lucide-react'
import type { Project, TranscriptLine } from '../types'

export const projects: Project[] = [
  { id: 'product-meeting', title: '产品周会讨论', date: '今天 14:32', duration: '1 小时 12 分', kind: '会议', status: '已完成', icon: Users },
  { id: 'ai-planning', title: 'AI 功能规划头脑风暴', date: '昨天 10:15', duration: '48 分钟', kind: '灵感', status: '已完成', icon: Lightbulb },
  { id: 'user-interview', title: '用户访谈 · 小张', date: '9 月 24 日 16:20', duration: '35 分钟', kind: '访谈', status: '已完成', icon: Mic2 },
  { id: 'course-notes', title: '行业趋势分享会', date: '9 月 22 日 21:03', duration: '1 小时 8 分', kind: '课程', status: '处理中', icon: BookOpen },
]

export const transcript: TranscriptLine[] = [
  { time: '00:00', speaker: '小天', initials: '天', color: 'blue', text: '大家好，今天我们主要讨论三个方面：新版本的产品规划、用户反馈的核心问题，以及下一阶段的市场策略。' },
  { time: '00:38', speaker: '同事 A', initials: 'A', color: 'violet', text: '我先从用户反馈说起。上周调研显示，用户最关心的是协作体验和 AI 生成的准确性，移动端的流畅度还有提升空间。' },
  { time: '02:15', speaker: '小天', initials: '天', color: 'green', text: '这两个问题会放在下个版本重点优化。启动速度和文档加载，是目前优先级最高的两项。' },
  { time: '03:02', speaker: '同事 B', initials: 'B', color: 'blue', text: '关于 AI 准确性，建议引入更多领域词库，并优化长文本的上下文理解能力。' },
  { time: '04:20', speaker: '小天', initials: '天', color: 'green', text: '同意。产品层面需要提供可编辑的引用来源，让用户能够快速核对与修正。' },
  { time: '05:18', speaker: '同事 C', initials: 'C', color: 'amber', text: '市场策略上建议先聚焦中小团队，把本地存储和数据安全作为核心差异点。' },
]

export const aiActions = [
  { title: '忠实整理', detail: '校对转写，优化语序但保留原意' },
  { title: '生成摘要', detail: '提炼核心内容与关键结论' },
  { title: '会议纪要', detail: '生成结构化纪要与决策' },
  { title: '提取待办', detail: '识别任务、负责人和时间' },
]
