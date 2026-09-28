use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row};
use uuid::Uuid;

use crate::{
    error::{CoreError, CoreResult},
    models::{
        AiDocumentRecord, CoreHealth, MediaProbe, NewProjectInput, NewTaskInput,
        NewTranscriptSegment, ProjectRecord, TaskRecord, TaskStatus, TranscriptSegment,
        TransitionTaskInput,
    },
    tasks::validate_transition,
};

pub struct Database {
    connection: Mutex<Connection>,
    path: PathBuf,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> CoreResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(&path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;",
        )?;
        connection.execute_batch(include_str!("../migrations/001_initial.sql"))?;
        connection.execute_batch(include_str!("../migrations/002_personal_workspace.sql"))?;

        Ok(Self {
            connection: Mutex::new(connection),
            path,
        })
    }

    fn connection(&self) -> CoreResult<std::sync::MutexGuard<'_, Connection>> {
        self.connection.lock().map_err(|_| {
            CoreError::new(
                "DATABASE_LOCK_POISONED",
                "本地数据库暂时不可用，请重新启动 VoiceDesk。",
            )
        })
    }

    pub fn restore_interrupted_tasks(&self) -> CoreResult<i64> {
        let now = Utc::now().to_rfc3339();
        let count = self.connection()?.execute(
            "UPDATE tasks SET status = 'paused', stage = '等待恢复', recovery_available = 1, updated_at = ?1 WHERE status = 'running'",
            params![now],
        )?;
        Ok(count as i64)
    }

    pub fn health(&self, recovered_task_count: i64) -> CoreResult<CoreHealth> {
        let connection = self.connection()?;
        let project_count = connection.query_row(
            "SELECT COUNT(*) FROM projects WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )?;
        let active_task_count = connection.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status IN ('queued', 'running', 'paused')",
            [],
            |row| row.get(0),
        )?;
        Ok(CoreHealth {
            version: env!("CARGO_PKG_VERSION").to_string(),
            database_path: self.path.display().to_string(),
            project_count,
            active_task_count,
            recovered_task_count,
        })
    }

    pub fn create_project(&self, input: NewProjectInput) -> CoreResult<ProjectRecord> {
        let title = input.title.trim();
        if title.is_empty() {
            return Err(CoreError::new(
                "INVALID_PROJECT_TITLE",
                "项目名称不能为空。",
            ));
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let language = input.language.unwrap_or_else(|| "zh-CN".to_string());
        self.connection()?.execute(
            "INSERT INTO projects (id, title, language, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, title, language, now],
        )?;
        Ok(ProjectRecord {
            id,
            title: title.to_string(),
            status: "draft".to_string(),
            language,
            duration_ms: 0,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn list_projects(&self) -> CoreResult<Vec<ProjectRecord>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, title, status, language, duration_ms, created_at, updated_at FROM projects WHERE deleted_at IS NULL ORDER BY updated_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ProjectRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                language: row.get(3)?,
                duration_ms: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_deleted_projects(&self) -> CoreResult<Vec<ProjectRecord>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, title, status, language, duration_ms, created_at, updated_at FROM projects WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ProjectRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                language: row.get(3)?,
                duration_ms: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn trash_project(&self, project_id: &str) -> CoreResult<()> {
        let connection = self.connection()?;
        let active_tasks: i64 = connection.query_row(
            "SELECT COUNT(*) FROM tasks WHERE project_id = ?1 AND status IN ('queued', 'running')",
            params![project_id],
            |row| row.get(0),
        )?;
        if active_tasks > 0 {
            return Err(CoreError::new(
                "PROJECT_BUSY",
                "项目仍有运行中的任务，请先取消或等待完成。",
            ));
        }
        let now = Utc::now().to_rfc3339();
        let changed = connection.execute(
            "UPDATE projects SET deleted_at = ?1, status = 'archived', updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![now, project_id],
        )?;
        if changed == 0 {
            return Err(CoreError::new("PROJECT_NOT_FOUND", "找不到对应项目。"));
        }
        Ok(())
    }

    pub fn restore_project(&self, project_id: &str) -> CoreResult<()> {
        let now = Utc::now().to_rfc3339();
        let changed = self.connection()?.execute(
            "UPDATE projects SET deleted_at = NULL, status = CASE WHEN EXISTS(SELECT 1 FROM transcript_segments WHERE project_id = ?1) THEN 'ready' ELSE 'draft' END, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NOT NULL",
            params![project_id, now],
        )?;
        if changed == 0 {
            return Err(CoreError::new(
                "PROJECT_NOT_FOUND",
                "回收站中找不到对应项目。",
            ));
        }
        Ok(())
    }

    pub fn get_project(&self, project_id: &str) -> CoreResult<Option<ProjectRecord>> {
        self.connection()?.query_row(
            "SELECT id, title, status, language, duration_ms, created_at, updated_at FROM projects WHERE id = ?1 AND deleted_at IS NULL",
            params![project_id],
            |row| Ok(ProjectRecord {
                id: row.get(0)?, title: row.get(1)?, status: row.get(2)?, language: row.get(3)?,
                duration_ms: row.get(4)?, created_at: row.get(5)?, updated_at: row.get(6)?,
            }),
        ).optional().map_err(Into::into)
    }

    pub fn register_media(&self, project_id: &str, probe: &MediaProbe) -> CoreResult<String> {
        let connection = self.connection()?;
        let project_exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND deleted_at IS NULL)",
            params![project_id],
            |row| row.get(0),
        )?;
        if !project_exists {
            return Err(CoreError::new("PROJECT_NOT_FOUND", "找不到对应项目。"));
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let kind = if probe.has_video { "video" } else { "audio" };
        connection.execute(
            "INSERT INTO media (id, project_id, source_path, display_name, kind, format_name, duration_ms, size_bytes, sample_rate, channels, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![id, project_id, probe.path, probe.display_name, kind, probe.format_name, probe.duration_ms, probe.size_bytes, probe.sample_rate, probe.channels, now],
        )?;
        connection.execute(
            "UPDATE projects SET duration_ms = MAX(duration_ms, ?1), updated_at = ?2 WHERE id = ?3",
            params![probe.duration_ms, now, project_id],
        )?;
        Ok(id)
    }

    pub fn project_media_path(&self, project_id: &str) -> CoreResult<String> {
        self.connection()?
            .query_row(
                "SELECT m.source_path FROM media m JOIN projects p ON p.id = m.project_id WHERE m.project_id = ?1 AND p.deleted_at IS NULL ORDER BY m.created_at ASC LIMIT 1",
                params![project_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| CoreError::new("MEDIA_NOT_FOUND", "此项目没有可播放的媒体。"))
    }

    pub fn create_task(&self, input: NewTaskInput) -> CoreResult<TaskRecord> {
        const ALLOWED_KINDS: &[&str] = &[
            "probe",
            "transcode",
            "waveform",
            "transcription",
            "diarization",
            "llm",
        ];
        if !ALLOWED_KINDS.contains(&input.kind.as_str()) {
            return Err(CoreError::new("INVALID_TASK_KIND", "不支持此任务类型。"));
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let input_json = input.input_json.unwrap_or_else(|| "{}".to_string());
        serde_json::from_str::<serde_json::Value>(&input_json).map_err(|error| {
            CoreError::new("INVALID_TASK_INPUT", "任务参数必须是有效 JSON。")
                .with_detail(error.to_string())
        })?;
        self.connection()?.execute(
            "INSERT INTO tasks (id, project_id, kind, status, input_json, created_at, updated_at) VALUES (?1, ?2, ?3, 'queued', ?4, ?5, ?5)",
            params![id, input.project_id, input.kind, input_json, now],
        )?;
        self.get_task(&id)?
            .ok_or_else(|| CoreError::new("TASK_CREATE_FAILED", "任务创建失败。"))
    }

    pub fn get_task(&self, task_id: &str) -> CoreResult<Option<TaskRecord>> {
        self.connection()?.query_row(
            "SELECT id, project_id, kind, status, stage, progress, recovery_available, input_json, output_json, error_code, error_message, attempt, created_at, updated_at FROM tasks WHERE id = ?1",
            params![task_id],
            read_task,
        ).optional().map_err(Into::into)
    }

    pub fn list_tasks(&self) -> CoreResult<Vec<TaskRecord>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, project_id, kind, status, stage, progress, recovery_available, input_json, output_json, error_code, error_message, attempt, created_at, updated_at FROM tasks ORDER BY updated_at DESC LIMIT 100",
        )?;
        let rows = statement.query_map([], read_task)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn begin_transcription(&self, project_id: &str, task_id: &str) -> CoreResult<()> {
        let now = Utc::now().to_rfc3339();
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM transcript_segments WHERE project_id = ?1",
            params![project_id],
        )?;
        transaction.execute(
            "UPDATE projects SET status = 'processing', updated_at = ?1 WHERE id = ?2",
            params![now, project_id],
        )?;
        transaction.execute(
            "UPDATE tasks SET output_json = NULL, error_code = NULL, error_message = NULL WHERE id = ?1",
            params![task_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn append_transcript_segment(
        &self,
        input: NewTranscriptSegment,
    ) -> CoreResult<TranscriptSegment> {
        if input.text.trim().is_empty() {
            return Err(CoreError::new(
                "EMPTY_TRANSCRIPT_SEGMENT",
                "识别结果不能为空。",
            ));
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        self.connection()?.execute(
            "INSERT INTO transcript_segments (id, project_id, task_id, sequence, start_ms, end_ms, raw_text, confidence, model, chunk_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
            params![id, input.project_id, input.task_id, input.sequence, input.start_ms, input.end_ms, input.text.trim(), input.confidence, input.model, input.chunk_id, now],
        )?;
        Ok(TranscriptSegment {
            id,
            project_id: input.project_id,
            task_id: Some(input.task_id),
            sequence: input.sequence,
            start_ms: input.start_ms,
            end_ms: input.end_ms,
            text: input.text.trim().to_string(),
            confidence: input.confidence,
            language: input.language,
            model: input.model,
            speaker: None,
            review_status: None,
        })
    }

    pub fn list_transcript_segments(&self, project_id: &str) -> CoreResult<Vec<TranscriptSegment>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT s.id, s.project_id, s.task_id, s.sequence, s.start_ms, s.end_ms, COALESCE(s.edited_text, s.raw_text), s.confidence, p.language, s.model, sp.display_name, r.status FROM transcript_segments s LEFT JOIN projects p ON p.id = s.project_id LEFT JOIN speakers sp ON sp.id = s.speaker_id LEFT JOIN segment_reviews r ON r.segment_id = s.id WHERE s.project_id = ?1 ORDER BY s.sequence ASC",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok(TranscriptSegment {
                id: row.get(0)?,
                project_id: row.get(1)?,
                task_id: row.get(2)?,
                sequence: row.get(3)?,
                start_ms: row.get(4)?,
                end_ms: row.get(5)?,
                text: row.get(6)?,
                confidence: row.get(7)?,
                language: row.get(8)?,
                model: row.get(9)?,
                speaker: row.get(10)?,
                review_status: row.get(11)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn update_transcript_segment(&self, segment_id: &str, text: &str) -> CoreResult<()> {
        if text.trim().is_empty() {
            return Err(CoreError::new(
                "EMPTY_TRANSCRIPT_SEGMENT",
                "转写文本不能为空。",
            ));
        }
        let changed = self.connection()?.execute(
            "UPDATE transcript_segments SET edited_text = ?1, updated_at = ?2 WHERE id = ?3",
            params![text.trim(), Utc::now().to_rfc3339(), segment_id],
        )?;
        if changed == 0 {
            return Err(CoreError::new("SEGMENT_NOT_FOUND", "找不到对应转写片段。"));
        }
        Ok(())
    }

    pub fn assign_speaker(
        &self,
        segment_id: &str,
        display_name: &str,
        apply_to_matching: bool,
    ) -> CoreResult<()> {
        let name = display_name.trim();
        if name.is_empty() || name.chars().count() > 40 {
            return Err(CoreError::new(
                "INVALID_SPEAKER_NAME",
                "说话人名称需为 1–40 个字符。",
            ));
        }
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let (project_id, current_speaker_id): (String, Option<String>) = transaction
            .query_row(
                "SELECT project_id, speaker_id FROM transcript_segments WHERE id = ?1",
                params![segment_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| CoreError::new("SEGMENT_NOT_FOUND", "找不到对应转写片段。"))?;

        if apply_to_matching {
            if let Some(speaker_id) = current_speaker_id {
                transaction.execute(
                    "UPDATE speakers SET display_name = ?1, updated_at = ?2 WHERE id = ?3 AND project_id = ?4",
                    params![name, Utc::now().to_rfc3339(), speaker_id, project_id],
                )?;
            } else {
                let speaker_id = Uuid::new_v4().to_string();
                let now = Utc::now().to_rfc3339();
                transaction.execute(
                    "INSERT INTO speakers (id, project_id, external_key, display_name, created_at, updated_at) VALUES (?1, ?2, ?1, ?3, ?4, ?4)",
                    params![speaker_id, project_id, name, now],
                )?;
                transaction.execute(
                    "UPDATE transcript_segments SET speaker_id = ?1, updated_at = ?2 WHERE project_id = ?3 AND speaker_id IS NULL",
                    params![speaker_id, Utc::now().to_rfc3339(), project_id],
                )?;
            }
        } else {
            let speaker_id: Option<String> = transaction
                .query_row(
                    "SELECT id FROM speakers WHERE project_id = ?1 AND display_name = ?2 LIMIT 1",
                    params![project_id, name],
                    |row| row.get(0),
                )
                .optional()?;
            let speaker_id = speaker_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            let now = Utc::now().to_rfc3339();
            transaction.execute(
                "INSERT OR IGNORE INTO speakers (id, project_id, external_key, display_name, created_at, updated_at) VALUES (?1, ?2, ?1, ?3, ?4, ?4)",
                params![speaker_id, project_id, name, now],
            )?;
            transaction.execute(
                "UPDATE transcript_segments SET speaker_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![speaker_id, Utc::now().to_rfc3339(), segment_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn get_project_text(&self, project_id: &str, kind: &str) -> CoreResult<String> {
        validate_project_text_kind(kind)?;
        let connection = self.connection()?;
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND deleted_at IS NULL)",
            params![project_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(CoreError::new("PROJECT_NOT_FOUND", "找不到对应项目。"));
        }
        Ok(connection
            .query_row(
                "SELECT content FROM project_texts WHERE project_id = ?1 AND kind = ?2",
                params![project_id, kind],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or_default())
    }

    pub fn save_project_text(&self, project_id: &str, kind: &str, content: &str) -> CoreResult<()> {
        validate_project_text_kind(kind)?;
        if content.chars().count() > 2_000_000 {
            return Err(CoreError::new(
                "PROJECT_TEXT_TOO_LARGE",
                "笔记或草稿不能超过 200 万字符。",
            ));
        }
        let changed = self.connection()?.execute(
            "INSERT INTO project_texts (project_id, kind, content, updated_at) SELECT id, ?2, ?3, ?4 FROM projects WHERE id = ?1 AND deleted_at IS NULL ON CONFLICT(project_id, kind) DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
            params![project_id, kind, content, Utc::now().to_rfc3339()],
        )?;
        if changed == 0 {
            return Err(CoreError::new("PROJECT_NOT_FOUND", "找不到对应项目。"));
        }
        Ok(())
    }

    pub fn set_segment_review(&self, segment_id: &str, status: &str) -> CoreResult<()> {
        if !["accepted", "ignored"].contains(&status) {
            return Err(CoreError::new(
                "INVALID_REVIEW_STATUS",
                "不支持此审核状态。",
            ));
        }
        let changed = self.connection()?.execute(
            "INSERT INTO segment_reviews (segment_id, status, updated_at) SELECT id, ?2, ?3 FROM transcript_segments WHERE id = ?1 ON CONFLICT(segment_id) DO UPDATE SET status = excluded.status, updated_at = excluded.updated_at",
            params![segment_id, status, Utc::now().to_rfc3339()],
        )?;
        if changed == 0 {
            return Err(CoreError::new("SEGMENT_NOT_FOUND", "找不到对应转写片段。"));
        }
        Ok(())
    }

    pub fn replace_transcript_text(
        &self,
        project_id: &str,
        find: &str,
        replacement: &str,
    ) -> CoreResult<i64> {
        if find.is_empty() {
            return Err(CoreError::new("EMPTY_SEARCH", "查找内容不能为空。"));
        }
        let now = Utc::now().to_rfc3339();
        let changed = self.connection()?.execute(
            "UPDATE transcript_segments SET edited_text = REPLACE(COALESCE(edited_text, raw_text), ?1, ?2), updated_at = ?3 WHERE project_id = ?4 AND INSTR(COALESCE(edited_text, raw_text), ?1) > 0",
            params![find, replacement, now, project_id],
        )?;
        Ok(changed as i64)
    }

    pub fn create_ai_document(
        &self,
        project_id: &str,
        document_type: &str,
        title: &str,
        content: &serde_json::Value,
        model: &str,
        prompt_version: &str,
    ) -> CoreResult<AiDocumentRecord> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let content_json = serde_json::to_string(content)?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO ai_documents (id, project_id, type, title, content, model, prompt_version, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![id, project_id, document_type, title, content_json, model, prompt_version, now],
        )?;
        for (block_key, sequence) in document_source_sequences(content) {
            let segment_id: Option<String> = transaction
                .query_row(
                    "SELECT id FROM transcript_segments WHERE project_id = ?1 AND sequence = ?2",
                    params![project_id, sequence],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(segment_id) = segment_id {
                transaction.execute(
                    "INSERT OR IGNORE INTO source_refs (document_id, segment_id, block_key) VALUES (?1, ?2, ?3)",
                    params![id, segment_id, block_key],
                )?;
            }
        }
        transaction.commit()?;
        drop(connection);
        self.get_ai_document(&id)?
            .ok_or_else(|| CoreError::new("DOCUMENT_CREATE_FAILED", "AI 文档保存失败。"))
    }

    pub fn get_ai_document(&self, document_id: &str) -> CoreResult<Option<AiDocumentRecord>> {
        let connection = self.connection()?;
        let document = connection.query_row(
            "SELECT id, project_id, type, title, content, model, prompt_version, created_at, updated_at FROM ai_documents WHERE id = ?1",
            params![document_id],
            read_ai_document,
        ).optional()?;
        document
            .map(|mut document| {
                document.source_segment_ids = list_document_sources(&connection, &document.id)?;
                Ok(document)
            })
            .transpose()
    }

    pub fn list_ai_documents(&self) -> CoreResult<Vec<AiDocumentRecord>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, project_id, type, title, content, model, prompt_version, created_at, updated_at FROM ai_documents ORDER BY updated_at DESC",
        )?;
        let rows = statement.query_map([], read_ai_document)?;
        let mut documents = rows.collect::<Result<Vec<_>, _>>()?;
        for document in &mut documents {
            document.source_segment_ids = list_document_sources(&connection, &document.id)?;
        }
        Ok(documents)
    }

    pub fn complete_llm_task(&self, task_id: &str, document_id: &str) -> CoreResult<TaskRecord> {
        let output = serde_json::to_string(&serde_json::json!({ "documentId": document_id }))?;
        let now = Utc::now().to_rfc3339();
        self.connection()?.execute(
            "UPDATE tasks SET status = 'completed', stage = 'AI 文档已生成', progress = 1, output_json = ?1, recovery_available = 0, updated_at = ?2, completed_at = ?2 WHERE id = ?3 AND status = 'running'",
            params![output, now, task_id],
        )?;
        self.get_task(task_id)?
            .ok_or_else(|| CoreError::new("TASK_NOT_FOUND", "找不到对应任务。"))
    }

    pub fn update_task_progress(
        &self,
        task_id: &str,
        progress: f64,
        stage: &str,
    ) -> CoreResult<TaskRecord> {
        let now = Utc::now().to_rfc3339();
        self.connection()?.execute(
            "UPDATE tasks SET progress = ?1, stage = ?2, updated_at = ?3 WHERE id = ?4 AND status = 'running'",
            params![progress.clamp(0.0, 0.999), stage, now, task_id],
        )?;
        self.get_task(task_id)?
            .ok_or_else(|| CoreError::new("TASK_NOT_FOUND", "找不到对应任务。"))
    }

    pub fn complete_transcription(
        &self,
        task_id: &str,
        project_id: &str,
        output_json: &str,
        duration_ms: i64,
        language: Option<&str>,
    ) -> CoreResult<TaskRecord> {
        let now = Utc::now().to_rfc3339();
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE tasks SET status = 'completed', stage = '转写完成', progress = 1, output_json = ?1, recovery_available = 0, updated_at = ?2, completed_at = ?2 WHERE id = ?3 AND status = 'running'",
            params![output_json, now, task_id],
        )?;
        transaction.execute(
            "UPDATE projects SET status = 'ready', duration_ms = MAX(duration_ms, ?1), language = COALESCE(?2, language), updated_at = ?3 WHERE id = ?4",
            params![duration_ms.max(0), language, now, project_id],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_task(task_id)?
            .ok_or_else(|| CoreError::new("TASK_NOT_FOUND", "找不到对应任务。"))
    }

    pub fn fail_transcription(
        &self,
        task_id: &str,
        project_id: &str,
        code: &str,
        message: &str,
    ) -> CoreResult<TaskRecord> {
        let now = Utc::now().to_rfc3339();
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE tasks SET status = 'failed', stage = '转写失败', recovery_available = 1, error_code = ?1, error_message = ?2, updated_at = ?3, completed_at = ?3 WHERE id = ?4 AND status = 'running'",
            params![code, message, now, task_id],
        )?;
        transaction.execute(
            "UPDATE projects SET status = 'failed', updated_at = ?1 WHERE id = ?2",
            params![now, project_id],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_task(task_id)?
            .ok_or_else(|| CoreError::new("TASK_NOT_FOUND", "找不到对应任务。"))
    }

    pub fn transition_task(&self, input: TransitionTaskInput) -> CoreResult<TaskRecord> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let current_status: String = transaction
            .query_row(
                "SELECT status FROM tasks WHERE id = ?1",
                params![input.task_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| CoreError::new("TASK_NOT_FOUND", "找不到对应任务。"))?;
        let current = TaskStatus::parse(&current_status)
            .ok_or_else(|| CoreError::new("INVALID_TASK_STATUS", "任务状态损坏。"))?;
        validate_transition(current, input.status)?;

        let progress = input
            .progress
            .unwrap_or_else(|| {
                if input.status == TaskStatus::Completed {
                    1.0
                } else {
                    0.0
                }
            })
            .clamp(0.0, 1.0);
        let stage = input
            .stage
            .unwrap_or_else(|| input.status.as_str().to_string());
        let now = Utc::now().to_rfc3339();
        let started_at = (input.status == TaskStatus::Running).then_some(now.as_str());
        let completed_at = matches!(
            input.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
        .then_some(now.as_str());
        let recovery_available = i64::from(input.status == TaskStatus::Paused);
        let attempt_delta =
            i64::from(current == TaskStatus::Failed && input.status == TaskStatus::Queued);
        transaction.execute(
            "UPDATE tasks SET status = ?1, stage = ?2, progress = ?3, recovery_available = ?4, error_code = ?5, error_message = ?6, attempt = attempt + ?7, updated_at = ?8, started_at = COALESCE(started_at, ?9), completed_at = ?10 WHERE id = ?11",
            params![input.status.as_str(), stage, progress, recovery_available, input.error_code, input.error_message, attempt_delta, now, started_at, completed_at, input.task_id],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_task(&input.task_id)?
            .ok_or_else(|| CoreError::new("TASK_NOT_FOUND", "找不到对应任务。"))
    }
}

fn validate_project_text_kind(kind: &str) -> CoreResult<()> {
    if ["notes", "draft"].contains(&kind) {
        Ok(())
    } else {
        Err(CoreError::new(
            "INVALID_PROJECT_TEXT_KIND",
            "只支持个人笔记或草稿。",
        ))
    }
}

fn document_source_sequences(content: &serde_json::Value) -> Vec<(String, i64)> {
    let mut result = Vec::new();
    for group in ["conclusions", "topics", "actionItems", "risks"] {
        let Some(items) = content.get(group).and_then(serde_json::Value::as_array) else {
            continue;
        };
        for (index, item) in items.iter().enumerate() {
            let Some(sequences) = item
                .get("sourceSequences")
                .and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            for sequence in sequences.iter().filter_map(serde_json::Value::as_i64) {
                result.push((format!("{group}:{index}"), sequence));
            }
        }
    }
    result
}

fn read_ai_document(row: &Row<'_>) -> rusqlite::Result<AiDocumentRecord> {
    let raw_content: String = row.get(4)?;
    let content = serde_json::from_str(&raw_content).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(AiDocumentRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        document_type: row.get(2)?,
        title: row.get(3)?,
        content,
        model: row.get(5)?,
        prompt_version: row.get(6)?,
        source_segment_ids: Vec::new(),
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn list_document_sources(connection: &Connection, document_id: &str) -> CoreResult<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT DISTINCT segment_id FROM source_refs WHERE document_id = ?1 ORDER BY segment_id",
    )?;
    let rows = statement.query_map(params![document_id], |row| row.get(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn read_task(row: &Row<'_>) -> rusqlite::Result<TaskRecord> {
    let raw_status: String = row.get(3)?;
    let status = TaskStatus::parse(&raw_status).ok_or_else(|| {
        rusqlite::Error::InvalidColumnType(3, "status".to_string(), rusqlite::types::Type::Text)
    })?;
    Ok(TaskRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        kind: row.get(2)?,
        status,
        stage: row.get(4)?,
        progress: row.get(5)?,
        recovery_available: row.get::<_, i64>(6)? != 0,
        input_json: row.get(7)?,
        output_json: row.get(8)?,
        error_code: row.get(9)?,
        error_message: row.get(10)?,
        attempt: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_database() -> (PathBuf, Database) {
        let root = std::env::temp_dir().join(format!("voicedesk-test-{}", Uuid::new_v4()));
        let database = Database::open(root.join("voicedesk.db")).expect("database opens");
        (root, database)
    }

    #[test]
    fn migrates_and_persists_projects() {
        let (root, database) = test_database();
        let project = database
            .create_project(NewProjectInput {
                title: "测试会议".into(),
                language: None,
            })
            .unwrap();
        assert_eq!(database.list_projects().unwrap()[0].id, project.id);
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn moves_projects_to_trash_and_restores_them() {
        let (root, database) = test_database();
        let project = database
            .create_project(NewProjectInput {
                title: "可恢复记录".into(),
                language: None,
            })
            .unwrap();
        database.trash_project(&project.id).unwrap();
        assert!(database.list_projects().unwrap().is_empty());
        assert_eq!(database.list_deleted_projects().unwrap().len(), 1);
        database.restore_project(&project.id).unwrap();
        assert_eq!(database.list_projects().unwrap().len(), 1);
        assert!(database.list_deleted_projects().unwrap().is_empty());
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn restores_running_tasks_as_paused() {
        let (root, database) = test_database();
        let task = database
            .create_task(NewTaskInput {
                project_id: None,
                kind: "probe".into(),
                input_json: None,
            })
            .unwrap();
        database
            .transition_task(TransitionTaskInput {
                task_id: task.id.clone(),
                status: TaskStatus::Running,
                stage: None,
                progress: None,
                error_code: None,
                error_message: None,
            })
            .unwrap();
        assert_eq!(database.restore_interrupted_tasks().unwrap(), 1);
        let restored = database.get_task(&task.id).unwrap().unwrap();
        assert_eq!(restored.status, TaskStatus::Paused);
        assert!(restored.recovery_available);
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn stores_transcript_segments_in_timeline_order() {
        let (root, database) = test_database();
        let project = database
            .create_project(NewProjectInput {
                title: "端到端测试".into(),
                language: Some("zh".into()),
            })
            .unwrap();
        let task = database
            .create_task(NewTaskInput {
                project_id: Some(project.id.clone()),
                kind: "transcription".into(),
                input_json: Some("{}".into()),
            })
            .unwrap();
        database
            .append_transcript_segment(NewTranscriptSegment {
                project_id: project.id.clone(),
                task_id: task.id,
                sequence: 0,
                start_ms: 0,
                end_ms: 1200,
                text: "大家好".into(),
                confidence: Some(0.95),
                language: Some("zh".into()),
                model: Some("small".into()),
                chunk_id: Some("0:0".into()),
            })
            .unwrap();
        let segments = database.list_transcript_segments(&project.id).unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "大家好");
        assert_eq!(segments[0].start_ms, 0);
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn stores_ai_document_with_source_references() {
        let (root, database) = test_database();
        let project = database
            .create_project(NewProjectInput {
                title: "来源测试".into(),
                language: Some("zh".into()),
            })
            .unwrap();
        let task = database
            .create_task(NewTaskInput {
                project_id: Some(project.id.clone()),
                kind: "transcription".into(),
                input_json: None,
            })
            .unwrap();
        let segment = database
            .append_transcript_segment(NewTranscriptSegment {
                project_id: project.id.clone(),
                task_id: task.id,
                sequence: 0,
                start_ms: 0,
                end_ms: 1000,
                text: "十月发布".into(),
                confidence: Some(0.9),
                language: Some("zh".into()),
                model: Some("small".into()),
                chunk_id: None,
            })
            .unwrap();
        let document = database
            .create_ai_document(
                &project.id,
                "meeting_notes",
                "会议纪要",
                &serde_json::json!({
                    "title": "会议纪要", "summary": "十月发布", "conclusions": [
                        { "text": "十月发布", "sourceSequences": [0] }
                    ], "topics": [], "actionItems": [], "risks": []
                }),
                "Qwen3-4B-Q4_K_M.gguf",
                "meeting-notes-v1",
            )
            .unwrap();
        assert_eq!(document.source_segment_ids, vec![segment.id]);
        assert_eq!(
            database.list_ai_documents().unwrap()[0]
                .prompt_version
                .as_deref(),
            Some("meeting-notes-v1")
        );
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn returns_registered_media_and_applies_manual_speaker_name() {
        let (root, database) = test_database();
        let project = database
            .create_project(NewProjectInput {
                title: "个人访谈".into(),
                language: Some("zh".into()),
            })
            .unwrap();
        database
            .register_media(
                &project.id,
                &MediaProbe {
                    path: "C:\\media\\interview.wav".into(),
                    display_name: "interview.wav".into(),
                    format_name: Some("wav".into()),
                    duration_ms: 2_000,
                    size_bytes: 200,
                    sample_rate: Some(16_000),
                    channels: Some(1),
                    has_video: false,
                },
            )
            .unwrap();
        assert_eq!(
            database.project_media_path(&project.id).unwrap(),
            "C:\\media\\interview.wav"
        );
        let task = database
            .create_task(NewTaskInput {
                project_id: Some(project.id.clone()),
                kind: "transcription".into(),
                input_json: None,
            })
            .unwrap();
        let segment = database
            .append_transcript_segment(NewTranscriptSegment {
                project_id: project.id.clone(),
                task_id: task.id,
                sequence: 0,
                start_ms: 0,
                end_ms: 1_000,
                text: "你好".into(),
                confidence: Some(0.7),
                language: Some("zh".into()),
                model: Some("small".into()),
                chunk_id: None,
            })
            .unwrap();
        database.assign_speaker(&segment.id, "我", true).unwrap();
        database
            .save_project_text(&project.id, "notes", "这是我的笔记")
            .unwrap();
        database
            .set_segment_review(&segment.id, "accepted")
            .unwrap();
        assert_eq!(
            database.list_transcript_segments(&project.id).unwrap()[0]
                .speaker
                .as_deref(),
            Some("我")
        );
        assert_eq!(
            database.list_transcript_segments(&project.id).unwrap()[0]
                .review_status
                .as_deref(),
            Some("accepted")
        );
        assert_eq!(
            database.get_project_text(&project.id, "notes").unwrap(),
            "这是我的笔记"
        );
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }
}
