use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: String,
    pub title: String,
    pub status: String,
    pub language: String,
    pub duration_ms: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewProjectInput {
    pub title: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "running" => Some(Self::Running),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: String,
    pub project_id: Option<String>,
    pub kind: String,
    pub status: TaskStatus,
    pub stage: String,
    pub progress: f64,
    pub recovery_available: bool,
    pub input_json: String,
    pub output_json: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub attempt: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTaskInput {
    pub project_id: Option<String>,
    pub kind: String,
    pub input_json: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionTaskInput {
    pub task_id: String,
    pub status: TaskStatus,
    pub stage: Option<String>,
    pub progress: Option<f64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaProbe {
    pub path: String,
    pub display_name: String,
    pub format_name: Option<String>,
    pub duration_ms: i64,
    pub size_bytes: i64,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub has_video: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterMediaInput {
    pub project_id: String,
    pub probe: MediaProbe,
}

impl<'de> Deserialize<'de> for MediaProbe {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Raw {
            path: String,
            display_name: String,
            format_name: Option<String>,
            duration_ms: i64,
            size_bytes: i64,
            sample_rate: Option<u32>,
            channels: Option<u32>,
            has_video: bool,
        }
        let raw = Raw::deserialize(deserializer)?;
        Ok(Self {
            path: raw.path,
            display_name: raw.display_name,
            format_name: raw.format_name,
            duration_ms: raw.duration_ms,
            size_bytes: raw.size_bytes,
            sample_rate: raw.sample_rate,
            channels: raw.channels,
            has_video: raw.has_video,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreHealth {
    pub version: String,
    pub database_path: String,
    pub project_count: i64,
    pub active_task_count: i64,
    pub recovered_task_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSnapshot {
    pub os_name: String,
    pub os_version: String,
    pub cpu_name: String,
    pub cpu_count: usize,
    pub total_memory_bytes: u64,
    pub available_memory_bytes: u64,
    pub available_disk_bytes: u64,
    pub ffmpeg_available: bool,
    pub ffprobe_available: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    pub path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub id: String,
    pub project_id: String,
    pub task_id: Option<String>,
    pub sequence: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub confidence: Option<f64>,
    pub language: Option<String>,
    pub model: Option<String>,
    pub speaker: Option<String>,
    pub review_status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTranscriptSegment {
    pub project_id: String,
    pub task_id: String,
    pub sequence: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub confidence: Option<f64>,
    pub language: Option<String>,
    pub model: Option<String>,
    pub chunk_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTranscriptionInput {
    pub task_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrHealth {
    pub available: bool,
    pub python_path: Option<String>,
    pub script_path: Option<String>,
    pub engine_installed: bool,
    pub engine_version: Option<String>,
    pub python_version: Option<String>,
    pub cuda_device_count: i64,
    pub model_directory: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingSession {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendRecordingInput {
    pub recording_id: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTranscriptInput {
    pub project_id: String,
    pub format: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTranscriptInput {
    pub segment_id: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignSpeakerInput {
    pub segment_id: String,
    pub display_name: String,
    pub apply_to_matching: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTextInput {
    pub project_id: String,
    pub kind: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentReviewInput {
    pub segment_id: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceTranscriptInput {
    pub project_id: String,
    pub find: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmHealth {
    pub available: bool,
    pub runtime_path: Option<String>,
    pub model_path: String,
    pub model_name: String,
    pub model_size_bytes: i64,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderStatus {
    pub provider: String,
    pub model: String,
    pub endpoint: String,
    pub configured: bool,
    pub local_available: bool,
    pub local_model_path: String,
    pub detail: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveApiKeyInput {
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateDocumentInput {
    pub project_id: String,
    pub document_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDocumentRecord {
    pub id: String,
    pub project_id: String,
    pub document_type: String,
    pub title: String,
    pub content: serde_json::Value,
    pub model: Option<String>,
    pub prompt_version: Option<String>,
    pub source_segment_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}
