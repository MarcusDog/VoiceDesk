PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS projects (
  id TEXT PRIMARY KEY NOT NULL,
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'processing', 'ready', 'failed', 'archived')),
  language TEXT NOT NULL DEFAULT 'zh-CN',
  duration_ms INTEGER NOT NULL DEFAULT 0 CHECK (duration_ms >= 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_projects_updated_at ON projects(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_projects_status ON projects(status) WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS media (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  source_path TEXT NOT NULL,
  display_name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('audio', 'video', 'recording')),
  format_name TEXT,
  duration_ms INTEGER NOT NULL DEFAULT 0 CHECK (duration_ms >= 0),
  size_bytes INTEGER NOT NULL DEFAULT 0 CHECK (size_bytes >= 0),
  sample_rate INTEGER,
  channels INTEGER,
  waveform_cache_path TEXT,
  fingerprint TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_media_project_id ON media(project_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_media_source_project ON media(project_id, source_path);

CREATE TABLE IF NOT EXISTS tasks (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('probe', 'transcode', 'waveform', 'transcription', 'diarization', 'llm')),
  status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'paused', 'completed', 'failed', 'cancelled')),
  stage TEXT NOT NULL DEFAULT 'queued',
  progress REAL NOT NULL DEFAULT 0 CHECK (progress >= 0 AND progress <= 1),
  recovery_available INTEGER NOT NULL DEFAULT 0 CHECK (recovery_available IN (0, 1)),
  input_json TEXT NOT NULL DEFAULT '{}',
  output_json TEXT,
  error_code TEXT,
  error_message TEXT,
  attempt INTEGER NOT NULL DEFAULT 0 CHECK (attempt >= 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_tasks_status_updated ON tasks(status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_tasks_project_id ON tasks(project_id, created_at DESC);

CREATE TABLE IF NOT EXISTS speakers (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  external_key TEXT NOT NULL,
  display_name TEXT NOT NULL,
  color TEXT NOT NULL DEFAULT 'blue',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(project_id, external_key)
);

CREATE TABLE IF NOT EXISTS transcript_segments (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  speaker_id TEXT REFERENCES speakers(id) ON DELETE SET NULL,
  sequence INTEGER NOT NULL CHECK (sequence >= 0),
  start_ms INTEGER NOT NULL CHECK (start_ms >= 0),
  end_ms INTEGER NOT NULL CHECK (end_ms >= start_ms),
  raw_text TEXT NOT NULL,
  edited_text TEXT,
  confidence REAL,
  model TEXT,
  chunk_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(project_id, sequence)
);

CREATE INDEX IF NOT EXISTS idx_segments_timeline ON transcript_segments(project_id, start_ms);
CREATE INDEX IF NOT EXISTS idx_segments_chunk ON transcript_segments(task_id, chunk_id);

CREATE TABLE IF NOT EXISTS ai_documents (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  type TEXT NOT NULL,
  title TEXT NOT NULL,
  content TEXT NOT NULL DEFAULT '',
  model TEXT,
  prompt_version TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_documents_project ON ai_documents(project_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS source_refs (
  document_id TEXT NOT NULL REFERENCES ai_documents(id) ON DELETE CASCADE,
  segment_id TEXT NOT NULL REFERENCES transcript_segments(id) ON DELETE CASCADE,
  block_key TEXT NOT NULL,
  quote_start INTEGER,
  quote_end INTEGER,
  PRIMARY KEY(document_id, segment_id, block_key)
);

CREATE INDEX IF NOT EXISTS idx_source_refs_segment ON source_refs(segment_id);

PRAGMA user_version = 1;
