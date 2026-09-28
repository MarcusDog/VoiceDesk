use std::io::Write;

use chrono::Utc;
use serde_json::json;
use tauri::{AppHandle, Emitter, State};

use crate::{
    asr,
    error::{CoreError, CoreResult},
    llm, media,
    models::{
        AiDocumentRecord, AiProviderStatus, AppendRecordingInput, AsrHealth, AssignSpeakerInput,
        CoreHealth, DiagnosticReport, ExportTranscriptInput, GenerateDocumentInput, LlmHealth,
        MediaProbe, NewProjectInput, NewTaskInput, ProjectRecord, ProjectTextInput,
        RecordingSession, RegisterMediaInput, ReplaceTranscriptInput, SaveApiKeyInput,
        SegmentReviewInput, StartTranscriptionInput, SystemSnapshot, TaskRecord, TranscriptSegment,
        TransitionTaskInput, UpdateTranscriptInput,
    },
    state::CoreState,
    system,
};

#[tauri::command]
pub fn core_health(state: State<'_, CoreState>) -> CoreResult<CoreHealth> {
    state.database.health(state.recovered_task_count())
}

#[tauri::command]
pub fn list_projects(state: State<'_, CoreState>) -> CoreResult<Vec<ProjectRecord>> {
    state.database.list_projects()
}

#[tauri::command]
pub fn list_deleted_projects(state: State<'_, CoreState>) -> CoreResult<Vec<ProjectRecord>> {
    state.database.list_deleted_projects()
}

#[tauri::command]
pub fn trash_project(project_id: String, state: State<'_, CoreState>) -> CoreResult<()> {
    state.database.trash_project(&project_id)
}

#[tauri::command]
pub fn restore_project(project_id: String, state: State<'_, CoreState>) -> CoreResult<()> {
    state.database.restore_project(&project_id)
}

#[tauri::command]
pub fn create_project(
    input: NewProjectInput,
    state: State<'_, CoreState>,
) -> CoreResult<ProjectRecord> {
    state.database.create_project(input)
}

#[tauri::command]
pub fn register_media(
    input: RegisterMediaInput,
    state: State<'_, CoreState>,
) -> CoreResult<String> {
    state
        .database
        .register_media(&input.project_id, &input.probe)
}

#[tauri::command]
pub async fn probe_media(path: String) -> CoreResult<MediaProbe> {
    tauri::async_runtime::spawn_blocking(move || media::probe_media(&path))
        .await
        .map_err(|error| {
            CoreError::new("WORKER_JOIN_FAILED", "媒体分析任务意外中断。")
                .with_detail(error.to_string())
        })?
}

#[tauri::command]
pub async fn cache_waveform(path: String, state: State<'_, CoreState>) -> CoreResult<String> {
    let cache_directory = state.cache_directory.join("waveforms");
    tauri::async_runtime::spawn_blocking(move || media::cache_waveform(&path, &cache_directory))
        .await
        .map_err(|error| {
            CoreError::new("WORKER_JOIN_FAILED", "波形任务意外中断。")
                .with_detail(error.to_string())
        })?
        .map(|path| path.display().to_string())
}

#[tauri::command]
pub async fn prepare_audio(path: String, state: State<'_, CoreState>) -> CoreResult<String> {
    let cache_directory = state.cache_directory.join("audio");
    tauri::async_runtime::spawn_blocking(move || media::transcode_to_pcm(&path, &cache_directory))
        .await
        .map_err(|error| {
            CoreError::new("WORKER_JOIN_FAILED", "音频准备任务意外中断。")
                .with_detail(error.to_string())
        })?
        .map(|path| path.display().to_string())
}

#[tauri::command]
pub async fn prepare_project_playback(
    project_id: String,
    state: State<'_, CoreState>,
) -> CoreResult<String> {
    let source = state.database.project_media_path(&project_id)?;
    let cache_directory = state.cache_directory.join("playback");
    tauri::async_runtime::spawn_blocking(move || media::prepare_playback(&source, &cache_directory))
        .await
        .map_err(|error| {
            CoreError::new("WORKER_JOIN_FAILED", "媒体播放准备任务意外中断。")
                .with_detail(error.to_string())
        })?
        .map(|path| path.display().to_string())
}

#[tauri::command]
pub fn enqueue_task(
    input: NewTaskInput,
    app: AppHandle,
    state: State<'_, CoreState>,
) -> CoreResult<TaskRecord> {
    let task = state.database.create_task(input)?;
    let _ = app.emit("core://task-updated", &task);
    Ok(task)
}

#[tauri::command]
pub fn list_tasks(state: State<'_, CoreState>) -> CoreResult<Vec<TaskRecord>> {
    state.database.list_tasks()
}

#[tauri::command]
pub fn list_transcript_segments(
    project_id: String,
    state: State<'_, CoreState>,
) -> CoreResult<Vec<TranscriptSegment>> {
    state.database.list_transcript_segments(&project_id)
}

#[tauri::command]
pub fn assign_speaker(input: AssignSpeakerInput, state: State<'_, CoreState>) -> CoreResult<()> {
    state.database.assign_speaker(
        &input.segment_id,
        &input.display_name,
        input.apply_to_matching,
    )
}

#[tauri::command]
pub fn get_project_text(
    project_id: String,
    kind: String,
    state: State<'_, CoreState>,
) -> CoreResult<String> {
    state.database.get_project_text(&project_id, &kind)
}

#[tauri::command]
pub fn save_project_text(input: ProjectTextInput, state: State<'_, CoreState>) -> CoreResult<()> {
    state
        .database
        .save_project_text(&input.project_id, &input.kind, &input.content)
}

#[tauri::command]
pub fn set_segment_review(
    input: SegmentReviewInput,
    state: State<'_, CoreState>,
) -> CoreResult<()> {
    state
        .database
        .set_segment_review(&input.segment_id, &input.status)
}

#[tauri::command]
pub fn asr_health(app: AppHandle, state: State<'_, CoreState>) -> AsrHealth {
    asr::health(&app, &state)
}

#[tauri::command]
pub fn llm_health(app: AppHandle, state: State<'_, CoreState>) -> LlmHealth {
    llm::health(&app, &state)
}

#[tauri::command]
pub fn ai_provider_status(state: State<'_, CoreState>) -> CoreResult<AiProviderStatus> {
    llm::provider_status(&state)
}

#[tauri::command]
pub fn save_minimax_api_key(input: SaveApiKeyInput) -> CoreResult<AiProviderStatus> {
    llm::save_minimax_api_key(&input.api_key)?;
    llm::provider_status_without_state()
}

#[tauri::command]
pub fn delete_minimax_api_key() -> CoreResult<()> {
    llm::delete_minimax_api_key()
}

#[tauri::command]
pub async fn test_minimax_connection() -> CoreResult<String> {
    tauri::async_runtime::spawn_blocking(llm::test_minimax_connection)
        .await
        .map_err(|error| {
            CoreError::new("MINIMAX_TEST_WORKER_FAILED", "MiniMax 连接测试意外中断。")
                .with_detail(error.to_string())
        })?
}

#[tauri::command]
pub async fn download_llm_model(app: AppHandle, state: State<'_, CoreState>) -> CoreResult<String> {
    let destination = state
        .app_data_directory
        .join("models/llm")
        .join("Qwen3-4B-Q4_K_M.gguf");
    tauri::async_runtime::spawn_blocking(move || llm::download_model(&app, &destination))
        .await
        .map_err(|error| {
            CoreError::new("MODEL_DOWNLOAD_WORKER_FAILED", "模型下载任务意外中断。")
                .with_detail(error.to_string())
        })?
}

#[tauri::command]
pub fn start_transcription(
    input: StartTranscriptionInput,
    app: AppHandle,
    state: State<'_, CoreState>,
) -> CoreResult<TaskRecord> {
    asr::start(input.task_id, app, &state)
}

#[tauri::command]
pub fn cancel_transcription(
    task_id: String,
    app: AppHandle,
    state: State<'_, CoreState>,
) -> CoreResult<TaskRecord> {
    asr::cancel(&task_id, &app, &state)
}

#[tauri::command]
pub fn generate_document(
    input: GenerateDocumentInput,
    app: AppHandle,
    state: State<'_, CoreState>,
) -> CoreResult<TaskRecord> {
    llm::start(input, app, &state)
}

#[tauri::command]
pub fn cancel_document_generation(
    task_id: String,
    app: AppHandle,
    state: State<'_, CoreState>,
) -> CoreResult<TaskRecord> {
    llm::cancel(&task_id, &app, &state)
}

#[tauri::command]
pub fn list_ai_documents(state: State<'_, CoreState>) -> CoreResult<Vec<AiDocumentRecord>> {
    state.database.list_ai_documents()
}

#[tauri::command]
pub fn begin_recording(state: State<'_, CoreState>) -> CoreResult<RecordingSession> {
    let id = uuid::Uuid::new_v4().to_string();
    let display_name = format!("录音-{}.webm", Utc::now().format("%Y%m%d-%H%M%S"));
    let directory = state.app_data_directory.join("recordings");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(&display_name);
    std::fs::File::create(&path)?;
    state
        .recordings
        .lock()
        .map_err(|_| CoreError::new("RECORDING_LOCKED", "录音写入器暂时不可用。"))?
        .insert(id.clone(), path);
    Ok(RecordingSession { id, display_name })
}

#[tauri::command]
pub fn append_recording_chunk(
    input: AppendRecordingInput,
    state: State<'_, CoreState>,
) -> CoreResult<()> {
    if input.bytes.len() > 16 * 1024 * 1024 {
        return Err(CoreError::new(
            "RECORDING_CHUNK_TOO_LARGE",
            "录音分块超过 16 MB。",
        ));
    }
    let path = state
        .recordings
        .lock()
        .map_err(|_| CoreError::new("RECORDING_LOCKED", "录音写入器暂时不可用。"))?
        .get(&input.recording_id)
        .cloned()
        .ok_or_else(|| CoreError::new("RECORDING_NOT_FOUND", "找不到当前录音会话。"))?;
    std::fs::OpenOptions::new()
        .append(true)
        .open(path)?
        .write_all(&input.bytes)?;
    Ok(())
}

#[tauri::command]
pub fn finish_recording(
    recording_id: String,
    state: State<'_, CoreState>,
) -> CoreResult<MediaProbe> {
    let path = state
        .recordings
        .lock()
        .map_err(|_| CoreError::new("RECORDING_LOCKED", "录音写入器暂时不可用。"))?
        .remove(&recording_id)
        .ok_or_else(|| CoreError::new("RECORDING_NOT_FOUND", "找不到当前录音会话。"))?;
    let metadata = std::fs::metadata(&path)?;
    if metadata.len() == 0 {
        let _ = std::fs::remove_file(&path);
        return Err(CoreError::new("EMPTY_RECORDING", "录音内容为空。"));
    }
    Ok(MediaProbe {
        display_name: path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "本地录音.webm".to_string()),
        path: path.display().to_string(),
        format_name: Some("webm".to_string()),
        duration_ms: 0,
        size_bytes: metadata.len() as i64,
        sample_rate: None,
        channels: None,
        has_video: false,
    })
}

#[tauri::command]
pub fn cancel_recording(recording_id: String, state: State<'_, CoreState>) -> CoreResult<()> {
    let path = state
        .recordings
        .lock()
        .map_err(|_| CoreError::new("RECORDING_LOCKED", "录音写入器暂时不可用。"))?
        .remove(&recording_id);
    if let Some(path) = path {
        if path.is_file() {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn update_transcript_segment(
    input: UpdateTranscriptInput,
    state: State<'_, CoreState>,
) -> CoreResult<()> {
    state
        .database
        .update_transcript_segment(&input.segment_id, &input.text)
}

#[tauri::command]
pub fn replace_transcript_text(
    input: ReplaceTranscriptInput,
    state: State<'_, CoreState>,
) -> CoreResult<i64> {
    state
        .database
        .replace_transcript_text(&input.project_id, &input.find, &input.replacement)
}

fn export_timestamp(milliseconds: i64, separator: char) -> String {
    let milliseconds = milliseconds.max(0);
    let hours = milliseconds / 3_600_000;
    let minutes = milliseconds / 60_000 % 60;
    let seconds = milliseconds / 1_000 % 60;
    let fraction = milliseconds % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02}{separator}{fraction:03}")
}

#[tauri::command]
pub fn export_transcript(
    input: ExportTranscriptInput,
    state: State<'_, CoreState>,
) -> CoreResult<String> {
    let segments = state.database.list_transcript_segments(&input.project_id)?;
    if segments.is_empty() {
        return Err(CoreError::new(
            "EMPTY_TRANSCRIPT",
            "当前项目没有可导出的转写内容。",
        ));
    }
    let content = match input.format.as_str() {
        "txt" => segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n"),
        "md" => segments
            .iter()
            .map(|segment| {
                format!(
                    "**[{}] {}**  \n{}",
                    export_timestamp(segment.start_ms, '.'),
                    segment.speaker.as_deref().unwrap_or("说话人"),
                    segment.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
        "srt" => segments
            .iter()
            .enumerate()
            .map(|(index, segment)| {
                format!(
                    "{}\n{} --> {}\n{}",
                    index + 1,
                    export_timestamp(segment.start_ms, ','),
                    export_timestamp(segment.end_ms, ','),
                    segment.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
        "vtt" => format!(
            "WEBVTT\n\n{}",
            segments
                .iter()
                .map(|segment| format!(
                    "{} --> {}\n{}",
                    export_timestamp(segment.start_ms, '.'),
                    export_timestamp(segment.end_ms, '.'),
                    segment.text
                ))
                .collect::<Vec<_>>()
                .join("\n\n")
        ),
        "json" => serde_json::to_string_pretty(&segments)?,
        _ => {
            return Err(CoreError::new(
                "INVALID_EXPORT_FORMAT",
                "不支持此导出格式。",
            ))
        }
    };
    std::fs::write(&input.path, content.as_bytes())?;
    Ok(input.path)
}

#[tauri::command]
pub fn transition_task(
    input: TransitionTaskInput,
    app: AppHandle,
    state: State<'_, CoreState>,
) -> CoreResult<TaskRecord> {
    let task = state.database.transition_task(input)?;
    let _ = app.emit("core://task-updated", &task);
    Ok(task)
}

#[tauri::command]
pub fn system_snapshot() -> SystemSnapshot {
    system::snapshot()
}

#[tauri::command]
pub fn export_diagnostics(state: State<'_, CoreState>) -> CoreResult<DiagnosticReport> {
    let created_at = Utc::now();
    let directory = state.app_data_directory.join("diagnostics");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!(
        "diagnostics-{}.json",
        created_at.format("%Y%m%d-%H%M%S")
    ));
    let report = json!({
        "createdAt": created_at.to_rfc3339(),
        "core": state.database.health(state.recovered_task_count())?,
        "system": system::snapshot(),
        "privacy": "No transcript text, media contents, or file contents are included."
    });
    std::fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    Ok(DiagnosticReport {
        path: path.display().to_string(),
        created_at: created_at.to_rfc3339(),
    })
}
