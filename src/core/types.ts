export type CoreError = {
  code: string
  message: string
  detail?: string
}

export type CoreHealth = {
  version: string
  databasePath: string
  projectCount: number
  activeTaskCount: number
  recoveredTaskCount: number
}

export type CoreConnection =
  | { mode: 'desktop'; health: CoreHealth }
  | { mode: 'browser'; health: null }
  | { mode: 'error'; health: null; error: CoreError }

export type ProjectRecord = {
  id: string
  title: string
  status: 'draft' | 'processing' | 'ready' | 'failed' | 'archived'
  language: string
  durationMs: number
  createdAt: string
  updatedAt: string
}

export type MediaProbe = {
  path: string
  displayName: string
  formatName?: string
  durationMs: number
  sizeBytes: number
  sampleRate?: number
  channels?: number
  hasVideo: boolean
}

export type TaskStatus = 'queued' | 'running' | 'paused' | 'completed' | 'failed' | 'cancelled'

export type TaskRecord = {
  id: string
  projectId?: string
  kind: 'probe' | 'transcode' | 'waveform' | 'transcription' | 'diarization' | 'llm'
  status: TaskStatus
  stage: string
  progress: number
  recoveryAvailable: boolean
  inputJson: string
  outputJson?: string
  errorCode?: string
  errorMessage?: string
  attempt: number
  createdAt: string
  updatedAt: string
}

export type SystemSnapshot = {
  osName: string
  osVersion: string
  cpuName: string
  cpuCount: number
  totalMemoryBytes: number
  availableMemoryBytes: number
  availableDiskBytes: number
  ffmpegAvailable: boolean
  ffprobeAvailable: boolean
}

export type TranscriptSegment = {
  id: string
  projectId: string
  taskId?: string
  sequence: number
  startMs: number
  endMs: number
  text: string
  confidence?: number
  language?: string
  model?: string
  speaker?: string
  reviewStatus?: 'accepted' | 'ignored'
}

export type AsrHealth = {
  available: boolean
  pythonPath?: string
  scriptPath?: string
  engineInstalled: boolean
  engineVersion?: string
  pythonVersion?: string
  cudaDeviceCount: number
  modelDirectory: string
  detail?: string
}

export type RecordingSession = {
  id: string
  displayName: string
}

export type LlmHealth = {
  available: boolean
  runtimePath?: string
  modelPath: string
  modelName: string
  modelSizeBytes: number
  detail?: string
}

export type AiProviderStatus = {
  provider: 'minimax'
  model: string
  endpoint: string
  configured: boolean
  localAvailable: boolean
  localModelPath: string
  detail?: string
}

export type AiDocumentType = 'meeting_notes' | 'executive_summary' | 'action_plan' | 'interview_insights'

export type AiDocumentItem = {
  text: string
  sourceSequences: number[]
}

export type AiActionItem = AiDocumentItem & {
  owner?: string | null
  due?: string | null
}

export type AiDocumentContent = {
  title: string
  summary: string
  conclusions: AiDocumentItem[]
  topics: AiDocumentItem[]
  actionItems: AiActionItem[]
  risks: AiDocumentItem[]
}

export type AiDocumentRecord = {
  id: string
  projectId: string
  documentType: AiDocumentType | string
  title: string
  content: AiDocumentContent
  model?: string
  promptVersion?: string
  sourceSegmentIds: string[]
  createdAt: string
  updatedAt: string
}
