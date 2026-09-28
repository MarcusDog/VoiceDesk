CREATE TABLE IF NOT EXISTS project_texts (
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('notes', 'draft')),
  content TEXT NOT NULL DEFAULT '',
  updated_at TEXT NOT NULL,
  PRIMARY KEY(project_id, kind)
);

CREATE TABLE IF NOT EXISTS segment_reviews (
  segment_id TEXT PRIMARY KEY NOT NULL REFERENCES transcript_segments(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('accepted', 'ignored')),
  updated_at TEXT NOT NULL
);

PRAGMA user_version = 2;
