import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { AiDocumentRecord, AiDocumentType, AiProviderStatus, AsrHealth, CoreConnection, CoreError, CoreHealth, LlmHealth, MediaProbe, ProjectRecord, RecordingSession, SystemSnapshot, TaskRecord, TaskStatus, TranscriptSegment } from './types'

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown
  }
}

export const isDesktopRuntime = () => typeof window !== 'undefined' && Boolean(window.__TAURI_INTERNALS__)

const normalizeError = (value: unknown): CoreError => {
  if (typeof value === 'object' && value !== null && 'code' in value && 'message' in value) {
    const error = value as { code: unknown; message: unknown; detail?: unknown }
    return {
      code: String(error.code),
      message: String(error.message),
      detail: error.detail ? String(error.detail) : undefined,
    }
  }
  return { code: 'UNKNOWN_ERROR', message: '本地核心发生未知错误。', detail: String(value) }
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args)
  } catch (error) {
    throw normalizeError(error)
  }
}

export async function connectCore(): Promise<CoreConnection> {
  if (!isDesktopRuntime()) return { mode: 'browser', health: null }
  try {
    return { mode: 'desktop', health: await call<CoreHealth>('core_health') }
  } catch (error) {
    return { mode: 'error', health: null, error: normalizeError(error) }
  }
}

export async function pickMediaFiles(): Promise<string[] | null> {
  if (!isDesktopRuntime()) return null
  const selection = await open({
    multiple: true,
    directory: false,
    title: '选择要转写的音视频',
    filters: [{ name: '音视频', extensions: ['mp3', 'wav', 'm4a', 'aac', 'flac', 'mp4', 'mov', 'mkv'] }],
  })
  if (!selection) return []
  return Array.isArray(selection) ? selection : [selection]
}

export const probeMedia = (path: string) => call<MediaProbe>('probe_media', { path })
export const prepareAudio = (path: string) => call<string>('prepare_audio', { path })
export const prepareProjectPlayback = (projectId: string) => call<string>('prepare_project_playback', { projectId })
export const cacheWaveform = (path: string) => call<string>('cache_waveform', { path })
export const getSystemSnapshot = () => call<SystemSnapshot>('system_snapshot')
export const listProjects = () => call<ProjectRecord[]>('list_projects')
export const listDeletedProjects = () => call<ProjectRecord[]>('list_deleted_projects')
export const trashProject = (projectId: string) => call<void>('trash_project', { projectId })
export const restoreProject = (projectId: string) => call<void>('restore_project', { projectId })
export const listTasks = () => call<TaskRecord[]>('list_tasks')
export const listTranscriptSegments = (projectId: string) => call<TranscriptSegment[]>('list_transcript_segments', { projectId })
export const getAsrHealth = () => call<AsrHealth>('asr_health')
export const getLlmHealth = () => call<LlmHealth>('llm_health')
export const getAiProviderStatus = () => call<AiProviderStatus>('ai_provider_status')
export const saveMinimaxApiKey = (apiKey: string) =>
  call<AiProviderStatus>('save_minimax_api_key', { input: { apiKey } })
export const deleteMinimaxApiKey = () => call<void>('delete_minimax_api_key')
export const testMinimaxConnection = () => call<string>('test_minimax_connection')
export const downloadLlmModel = () => call<string>('download_llm_model')
export const listAiDocuments = () => call<AiDocumentRecord[]>('list_ai_documents')

export const createProject = (title: string, language = 'zh-CN') =>
  call<ProjectRecord>('create_project', { input: { title, language } })

export const registerMedia = (projectId: string, probe: MediaProbe) =>
  call<string>('register_media', { input: { projectId, probe } })

export const enqueueTask = (kind: TaskRecord['kind'], projectId?: string, input: Record<string, unknown> = {}) =>
  call<TaskRecord>('enqueue_task', { input: { kind, projectId, inputJson: JSON.stringify(input) } })

export const transitionTask = (taskId: string, status: TaskStatus, options: { stage?: string; progress?: number; errorCode?: string; errorMessage?: string } = {}) =>
  call<TaskRecord>('transition_task', { input: { taskId, status, ...options } })

export const startTranscription = (taskId: string) => call<TaskRecord>('start_transcription', { input: { taskId } })
export const cancelTranscription = (taskId: string) => call<TaskRecord>('cancel_transcription', { taskId })
export const generateDocument = (projectId: string, documentType: AiDocumentType = 'meeting_notes') =>
  call<TaskRecord>('generate_document', { input: { projectId, documentType } })
export const cancelDocumentGeneration = (taskId: string) => call<TaskRecord>('cancel_document_generation', { taskId })

export const onTaskUpdated = (handler: (task: TaskRecord) => void): Promise<UnlistenFn> =>
  listen<TaskRecord>('core://task-updated', event => handler(event.payload))

export const onTranscriptSegment = (handler: (segment: TranscriptSegment) => void): Promise<UnlistenFn> =>
  listen<TranscriptSegment>('core://transcript-segment', event => handler(event.payload))

export const onDocumentCreated = (handler: (document: AiDocumentRecord) => void): Promise<UnlistenFn> =>
  listen<AiDocumentRecord>('core://document-created', event => handler(event.payload))

export const onLlmDownloadProgress = (handler: (progress: { progress: number; stage: string; downloadedBytes?: number; totalBytes?: number }) => void): Promise<UnlistenFn> =>
  listen('core://llm-download-progress', event => handler(event.payload as { progress: number; stage: string; downloadedBytes?: number; totalBytes?: number }))

export const beginRecording = () => call<RecordingSession>('begin_recording')
export const appendRecordingChunk = (recordingId: string, bytes: Uint8Array) =>
  call<void>('append_recording_chunk', { input: { recordingId, bytes: Array.from(bytes) } })
export const finishRecording = (recordingId: string) => call<MediaProbe>('finish_recording', { recordingId })
export const cancelRecording = (recordingId: string) => call<void>('cancel_recording', { recordingId })
export const updateTranscriptSegment = (segmentId: string, text: string) =>
  call<void>('update_transcript_segment', { input: { segmentId, text } })
export const assignSpeaker = (segmentId: string, displayName: string, applyToMatching = true) =>
  call<void>('assign_speaker', { input: { segmentId, displayName, applyToMatching } })
export const getProjectText = (projectId: string, kind: 'notes' | 'draft') =>
  call<string>('get_project_text', { projectId, kind })
export const saveProjectText = (projectId: string, kind: 'notes' | 'draft', content: string) =>
  call<void>('save_project_text', { input: { projectId, kind, content } })
export const setSegmentReview = (segmentId: string, status: 'accepted' | 'ignored') =>
  call<void>('set_segment_review', { input: { segmentId, status } })
export const replaceTranscriptText = (projectId: string, find: string, replacement: string) =>
  call<number>('replace_transcript_text', { input: { projectId, find, replacement } })

export async function exportTranscript(projectId: string, title: string, format: 'md' | 'txt' | 'srt' | 'vtt' | 'json') {
  const path = await save({
    title: '导出转写',
    defaultPath: `${title.replace(/[<>:"/\\|?*]/g, '-')}.${format}`,
    filters: [{ name: format.toUpperCase(), extensions: [format] }],
  })
  if (!path) return null
  return call<string>('export_transcript', { input: { projectId, format, path } })
}

export const exportDiagnostics = () => call<{ path: string; createdAt: string }>('export_diagnostics')
