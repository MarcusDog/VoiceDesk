import { ArrowUpDown, Filter, MoreHorizontal, RotateCcw, Search, Trash2 } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { projects } from '../data/demo'
import { listDeletedProjects, listProjects, restoreProject, trashProject } from '../core/client'
import type { CoreConnection, ProjectRecord } from '../core/types'
import type { PageId } from '../types'

type LibraryPageProps = { onNavigate: (page: PageId) => void; onOpenProject: (projectId: string) => void; core: CoreConnection }

const formatDuration = (milliseconds: number) => {
  const totalMinutes = Math.round(milliseconds / 60000)
  return totalMinutes >= 60 ? `${Math.floor(totalMinutes / 60)}小时 ${totalMinutes % 60}分` : `${totalMinutes}分钟`
}

export function LibraryPage({ onNavigate, onOpenProject, core }: LibraryPageProps) {
  const [filter, setFilter] = useState('全部')
  const [query, setQuery] = useState('')
  const [nativeProjects, setNativeProjects] = useState<ProjectRecord[]>([])
  const [trashMode, setTrashMode] = useState(false)
  const [message, setMessage] = useState('')
  useEffect(() => {
    if (core.mode === 'desktop') void (trashMode ? listDeletedProjects() : listProjects()).then(setNativeProjects)
  }, [core.mode, trashMode])
  const refresh = async () => setNativeProjects(await (trashMode ? listDeletedProjects() : listProjects()))
  const changeTrashState = async (projectId: string) => {
    try {
      if (trashMode) {
        await restoreProject(projectId)
        setMessage('项目已恢复到全部记录。')
      } else {
        await trashProject(projectId)
        setMessage('项目已移到回收站，可随时恢复。')
      }
      await refresh()
    } catch (error) { setMessage((error as { message?: string }).message || '操作失败。') }
  }
  const visible = useMemo(() => projects.filter(project => (filter === '全部' || project.kind === filter) && project.title.includes(query)), [filter, query])
  return (
    <div className="page library-page">
      <div className="page-heading page-heading--between">
        <div className="heading-cluster"><span className="section-index">04</span><h1>{trashMode ? '回收站' : '全部记录'}</h1></div>
        <button className="primary-button" onClick={() => onNavigate('import')}>新建转写</button>
      </div>
      <div className="library-toolbar">
        <label className="search-field"><Search size={17} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索记录…" /></label>
        <div className="filter-tabs">
          {['全部', '会议', '访谈', '课程', '灵感'].map(item => <button key={item} className={filter === item ? 'is-active' : ''} disabled={core.mode === 'desktop' && item !== '全部'} title={core.mode === 'desktop' && item !== '全部' ? '类型标签将在项目分类功能完成后开放' : undefined} onClick={() => setFilter(item)}>{item}</button>)}
        </div>
        <button className="secondary-button compact" disabled title="复合筛选将在后续版本开放"><Filter size={16} /> 筛选</button>
        <button className="secondary-button compact" disabled title="当前固定按最近更新时间排序"><ArrowUpDown size={16} /> 最近更新</button>
        <button className="secondary-button compact library-trash-button" onClick={() => { setTrashMode(value => !value); setMessage('') }}>{trashMode ? <><RotateCcw size={16} /> 返回记录</> : <><Trash2 size={16} /> 回收站</>}</button>
      </div>
      {message && <div className="export-message">{message}</div>}
      <div className="library-table">
        <div className="library-table-head"><span>名称</span><span>类型</span><span>状态</span><span>更新时间</span><span>时长</span><span /></div>
        {nativeProjects.filter(project => project.title.includes(query)).map(project => (
          <div className="library-table-row" key={project.id} role="button" tabIndex={0} onClick={() => { if (!trashMode) onOpenProject(project.id) }} onKeyDown={event => { if (!trashMode && (event.key === 'Enter' || event.key === ' ')) onOpenProject(project.id) }}>
            <span className="library-name"><i><Search size={19} /></i><strong>{project.title}</strong></span>
            <span><em className="tag tag--会议">本地</em></span>
            <span><em className={`table-status ${project.status === 'processing' ? 'is-processing' : ''}`}><i />{project.status === 'processing' ? '处理中' : project.status === 'failed' ? '失败' : '已保存'}</em></span>
            <span>{new Date(project.updatedAt).toLocaleString('zh-CN')}</span><span>{formatDuration(project.durationMs)}</span><button className="library-row-action" aria-label={trashMode ? '恢复项目' : '移到回收站'} title={trashMode ? '恢复项目' : '移到回收站'} onClick={event => { event.stopPropagation(); void changeTrashState(project.id) }}>{trashMode ? <RotateCcw size={17} /> : <Trash2 size={17} />}</button>
          </div>
        ))}
        {core.mode !== 'desktop' && visible.map(({ id, title, date, duration, kind, status, icon: Icon }) => (
          <button className="library-table-row" key={id} onClick={() => onNavigate('workspace')}>
            <span className="library-name"><i><Icon size={19} /></i><strong>{title}</strong></span>
            <span><em className={`tag tag--${kind}`}>{kind}</em></span>
            <span><em className={`table-status ${status === '处理中' ? 'is-processing' : ''}`}><i />{status}</em></span>
            <span>{date}</span><span>{duration}</span><span><MoreHorizontal size={18} /></span>
          </button>
        ))}
        {!nativeProjects.filter(project => project.title.includes(query)).length && (core.mode === 'desktop' || !visible.length) && <div className="large-empty"><Search size={30} /><strong>没有找到相关记录</strong><span>试试其他关键词，或创建第一条转写。</span></div>}
      </div>
    </div>
  )
}
