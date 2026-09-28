use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    error::{CoreError, CoreResult},
    models::{
        AiProviderStatus, GenerateDocumentInput, LlmHealth, NewTaskInput, TaskRecord, TaskStatus,
        TransitionTaskInput,
    },
    state::CoreState,
};

const MODEL_NAME: &str = "Qwen3-4B-Q4_K_M.gguf";
const MODEL_SIZE_BYTES: u64 = 2_497_280_256;
const MODEL_SHA256: &str = "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5";
const MODEL_URLS: &[&str] = &[
    "https://hf-mirror.com/Qwen/Qwen3-4B-GGUF/resolve/main/Qwen3-4B-Q4_K_M.gguf",
    "https://huggingface.co/Qwen/Qwen3-4B-GGUF/resolve/main/Qwen3-4B-Q4_K_M.gguf?download=true",
];
const MINIMAX_ENDPOINT: &str = "https://api.minimaxi.com/v1/chat/completions";
const MINIMAX_MODEL: &str = "MiniMax-M3";
const CREDENTIAL_SERVICE: &str = "com.voicedesk.desktop";
const CREDENTIAL_USER: &str = "minimax-api-key";

#[derive(Debug)]
struct LlmPaths {
    sidecar_binary: Option<PathBuf>,
    python: Option<PathBuf>,
    script: Option<PathBuf>,
    llama_server: Option<PathBuf>,
    model: PathBuf,
    asr_models: PathBuf,
}

fn first_file(paths: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    paths.into_iter().find(|path| path.is_file())
}

fn installed_llama_server() -> Option<PathBuf> {
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)?
        .join("Microsoft/WinGet/Packages");
    std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .find_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            name.starts_with("ggml.llamacpp_")
                .then(|| entry.path().join("llama-server.exe"))
                .filter(|path| path.is_file())
        })
}

fn resolve_paths(app: &AppHandle, state: &CoreState) -> CoreResult<LlmPaths> {
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")));
    let resource_root = app.path().resource_dir().ok();
    let sidecar_binary = std::env::var_os("VOICEDESK_ASR_BINARY")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| {
            let mut candidates =
                vec![manifest_root.join("dist-asr/voicedesk-asr/voicedesk-asr.exe")];
            if let Some(root) = resource_root.as_ref() {
                candidates.push(root.join("dist-asr/voicedesk-asr/voicedesk-asr.exe"));
                candidates.push(root.join("_up_/dist-asr/voicedesk-asr/voicedesk-asr.exe"));
            }
            first_file(candidates)
        });
    let (python, script) = if sidecar_binary.is_some() {
        (None, None)
    } else {
        let python = first_file([
            manifest_root.join(".venv-asr/Scripts/python.exe"),
            manifest_root.join(".venv-asr/bin/python"),
        ])
        .ok_or_else(|| CoreError::new("LLM_SIDECAR_NOT_FOUND", "找不到本地 AI 任务运行器。"))?;
        let mut scripts = vec![manifest_root.join("engines/asr/server.py")];
        if let Some(root) = resource_root.as_ref() {
            scripts.push(root.join("engines/asr/server.py"));
            scripts.push(root.join("_up_/engines/asr/server.py"));
        }
        let script = first_file(scripts)
            .ok_or_else(|| CoreError::new("LLM_SCRIPT_NOT_FOUND", "找不到本地 AI 引擎脚本。"))?;
        (Some(python), Some(script))
    };

    let llama_server = std::env::var_os("VOICEDESK_LLAMA_SERVER")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| {
            let mut candidates = vec![manifest_root.join("runtime/llama/llama-server.exe")];
            if let Some(root) = resource_root.as_ref() {
                candidates.push(root.join("runtime/llama/llama-server.exe"));
                candidates.push(root.join("_up_/runtime/llama/llama-server.exe"));
            }
            first_file(candidates)
        })
        .or_else(installed_llama_server);
    let model = std::env::var_os("VOICEDESK_LLM_MODEL")
        .map(PathBuf::from)
        .unwrap_or_else(|| state.app_data_directory.join("models/llm").join(MODEL_NAME));

    Ok(LlmPaths {
        sidecar_binary,
        python,
        script,
        llama_server,
        model,
        asr_models: state.app_data_directory.join("models/asr"),
    })
}

fn hidden_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
}

fn sidecar_command(paths: &LlmPaths) -> CoreResult<Command> {
    if let Some(binary) = paths.sidecar_binary.as_ref() {
        return Ok(hidden_command(binary));
    }
    let mut command =
        hidden_command(paths.python.as_ref().ok_or_else(|| {
            CoreError::new("LLM_SIDECAR_NOT_FOUND", "找不到本地 AI 任务运行器。")
        })?);
    command.arg(
        paths
            .script
            .as_ref()
            .ok_or_else(|| CoreError::new("LLM_SCRIPT_NOT_FOUND", "找不到本地 AI 引擎脚本。"))?,
    );
    Ok(command)
}

fn credential_entry() -> CoreResult<keyring::Entry> {
    keyring::Entry::new(CREDENTIAL_SERVICE, CREDENTIAL_USER).map_err(|error| {
        CoreError::new(
            "CREDENTIAL_STORE_UNAVAILABLE",
            "无法访问 Windows 凭据管理器。",
        )
        .with_detail(error.to_string())
    })
}

fn minimax_api_key() -> CoreResult<Option<String>> {
    match credential_entry()?.get_password() {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(
            CoreError::new("CREDENTIAL_READ_FAILED", "无法读取 MiniMax API 密钥。")
                .with_detail(error.to_string()),
        ),
    }
}

pub fn save_minimax_api_key(api_key: &str) -> CoreResult<()> {
    let value = api_key.trim();
    if value.len() < 20 || value.len() > 512 || value.chars().any(char::is_whitespace) {
        return Err(CoreError::new(
            "INVALID_API_KEY",
            "API 密钥格式无效，请粘贴新生成的完整 MiniMax 密钥。",
        ));
    }
    credential_entry()?.set_password(value).map_err(|error| {
        CoreError::new(
            "CREDENTIAL_WRITE_FAILED",
            "无法将 MiniMax API 密钥保存到 Windows 凭据管理器。",
        )
        .with_detail(error.to_string())
    })
}

pub fn delete_minimax_api_key() -> CoreResult<()> {
    match credential_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(CoreError::new(
            "CREDENTIAL_DELETE_FAILED",
            "无法从 Windows 凭据管理器删除 MiniMax API 密钥。",
        )
        .with_detail(error.to_string())),
    }
}

fn local_model_path(state: Option<&CoreState>) -> PathBuf {
    state
        .map(|value| value.app_data_directory.join("models/llm").join(MODEL_NAME))
        .or_else(|| {
            std::env::var_os("APPDATA").map(|root| {
                PathBuf::from(root)
                    .join("com.voicedesk.desktop/models/llm")
                    .join(MODEL_NAME)
            })
        })
        .unwrap_or_else(|| PathBuf::from(MODEL_NAME))
}

fn build_provider_status(state: Option<&CoreState>) -> CoreResult<AiProviderStatus> {
    let configured = minimax_api_key()?.is_some();
    let model_path = local_model_path(state);
    let local_available = std::fs::metadata(&model_path)
        .map(|value| value.len() == MODEL_SIZE_BYTES)
        .unwrap_or(false);
    Ok(AiProviderStatus {
        provider: "minimax".to_string(),
        model: MINIMAX_MODEL.to_string(),
        endpoint: MINIMAX_ENDPOINT.to_string(),
        configured,
        local_available,
        local_model_path: model_path.display().to_string(),
        detail: (!configured).then(|| {
            "请先撤销已暴露的旧密钥，再在此设备上保存新生成的 MiniMax API 密钥。".to_string()
        }),
    })
}

pub fn provider_status(state: &CoreState) -> CoreResult<AiProviderStatus> {
    build_provider_status(Some(state))
}

pub fn provider_status_without_state() -> CoreResult<AiProviderStatus> {
    build_provider_status(None)
}

pub fn test_minimax_connection() -> CoreResult<String> {
    let api_key = minimax_api_key()?.ok_or_else(|| {
        CoreError::new("MINIMAX_API_KEY_MISSING", "请先保存新的 MiniMax API 密钥。")
    })?;
    let client = reqwest::blocking::Client::builder()
        .user_agent("VoiceDesk/0.4")
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|error| {
            CoreError::new("MINIMAX_CLIENT_FAILED", "无法初始化 MiniMax 连接。")
                .with_detail(error.to_string())
        })?;
    let response = client
        .post(MINIMAX_ENDPOINT)
        .bearer_auth(api_key)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(
            json!({
                "model": MINIMAX_MODEL,
                "messages": [{"role": "user", "content": "只回复 OK"}],
                "thinking": {"type": "disabled"},
                "max_completion_tokens": 8,
                "stream": false
            })
            .to_string(),
        )
        .send()
        .map_err(|error| {
            CoreError::new("MINIMAX_CONNECTION_FAILED", "无法连接 MiniMax API。")
                .with_detail(error.to_string())
        })?;
    if !response.status().is_success() {
        let status = response.status();
        return Err(CoreError::new(
            "MINIMAX_AUTH_FAILED",
            "MiniMax API 拒绝了请求，请检查新密钥、账户额度与网络。",
        )
        .with_detail(format!("HTTP {status}")));
    }
    Ok(format!("{MINIMAX_MODEL} 连接成功"))
}

pub fn health(app: &AppHandle, state: &CoreState) -> LlmHealth {
    let model = state.app_data_directory.join("models/llm").join(MODEL_NAME);
    match resolve_paths(app, state) {
        Ok(paths) => {
            let model_size_bytes = std::fs::metadata(&paths.model)
                .map(|value| value.len() as i64)
                .unwrap_or(0);
            let available = paths.llama_server.is_some()
                && paths.model.is_file()
                && model_size_bytes == MODEL_SIZE_BYTES as i64;
            LlmHealth {
                available,
                runtime_path: paths
                    .llama_server
                    .as_ref()
                    .map(|path| path.display().to_string()),
                model_path: paths.model.display().to_string(),
                model_name: MODEL_NAME.to_string(),
                model_size_bytes,
                detail: (!available).then(|| {
                    if paths.llama_server.is_none() {
                        "未找到 llama.cpp；MiniMax-M3 仍可工作，本地 Qwen 备用不可用。".to_string()
                    } else {
                        "本地 Qwen 备用模型尚未下载完整。".to_string()
                    }
                }),
            }
        }
        Err(error) => LlmHealth {
            available: false,
            runtime_path: None,
            model_path: model.display().to_string(),
            model_name: MODEL_NAME.to_string(),
            model_size_bytes: std::fs::metadata(model)
                .map(|value| value.len() as i64)
                .unwrap_or(0),
            detail: Some(error.message),
        },
    }
}

pub fn download_model(app: &AppHandle, destination: &Path) -> CoreResult<String> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::metadata(destination)
        .map(|value| value.len())
        .unwrap_or(0)
        == MODEL_SIZE_BYTES
        && sha256_file(destination)? == MODEL_SHA256
    {
        return Ok(destination.display().to_string());
    }
    let partial = destination.with_extension("gguf.part");
    let partial_size = std::fs::metadata(&partial)
        .map(|value| value.len())
        .unwrap_or(0);
    if partial_size == MODEL_SIZE_BYTES {
        if sha256_file(&partial)? == MODEL_SHA256 {
            std::fs::rename(&partial, destination)?;
            return Ok(destination.display().to_string());
        }
        std::fs::remove_file(&partial)?;
    } else if partial_size > MODEL_SIZE_BYTES {
        std::fs::remove_file(&partial)?;
    }
    let mut last_error = String::new();
    for (index, url) in MODEL_URLS.iter().enumerate() {
        let source = if index == 0 { "镜像源" } else { "官方源" };
        let source_timeout = if index == 0 {
            std::time::Duration::from_secs(2 * 60 * 60)
        } else {
            std::time::Duration::from_secs(5 * 60)
        };
        let client = reqwest::blocking::Client::builder()
            .user_agent("VoiceDesk/0.4")
            .connect_timeout(std::time::Duration::from_secs(30))
            .timeout(source_timeout)
            .build()
            .map_err(|error| {
                CoreError::new("MODEL_DOWNLOAD_CLIENT_FAILED", "无法初始化模型下载器。")
                    .with_detail(error.to_string())
            })?;
        let _ = app.emit(
            "core://llm-download-progress",
            json!({ "progress": 0.0, "stage": format!("正在连接{source}") }),
        );
        let offset = std::fs::metadata(&partial)
            .map(|value| value.len())
            .unwrap_or(0);
        let mut request = client.get(*url);
        if offset > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={offset}-"));
        }
        let response = match request
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
        {
            Ok(response) => response,
            Err(error) => {
                last_error = error.to_string();
                continue;
            }
        };
        let accepts_resume =
            offset > 0 && response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        let starting_at = if accepts_resume { offset } else { 0 };
        let total = starting_at
            + response
                .content_length()
                .unwrap_or(MODEL_SIZE_BYTES - starting_at);
        let mut reader = response;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(accepts_resume)
            .truncate(!accepts_resume)
            .open(&partial)?;
        let mut buffer = vec![0_u8; 1024 * 1024];
        let mut downloaded = starting_at;
        let mut stream_error = None;
        loop {
            let count = match reader.read(&mut buffer) {
                Ok(count) => count,
                Err(error) => {
                    stream_error = Some(error.to_string());
                    break;
                }
            };
            if count == 0 {
                break;
            }
            file.write_all(&buffer[..count])?;
            downloaded += count as u64;
            let _ = app.emit(
                "core://llm-download-progress",
                json!({
                    "progress": (downloaded as f64 / total.max(1) as f64).clamp(0.0, 0.999),
                    "downloadedBytes": downloaded,
                    "totalBytes": total,
                    "stage": format!("正在从{source}下载 Qwen3-4B"),
                }),
            );
        }
        file.sync_all()?;
        if let Some(error) = stream_error {
            last_error = error;
            continue;
        }
        if downloaded < 1_000_000_000 {
            last_error = format!("downloaded file was unexpectedly small: {downloaded}");
            continue;
        }
        let _ = app.emit(
            "core://llm-download-progress",
            json!({ "progress": 0.999, "stage": "正在校验模型完整性" }),
        );
        let actual_hash = sha256_file(&partial)?;
        if actual_hash != MODEL_SHA256 {
            last_error = format!("SHA-256 mismatch: expected {MODEL_SHA256}, got {actual_hash}");
            let _ = std::fs::remove_file(&partial);
            continue;
        }
        std::fs::rename(&partial, destination)?;
        let _ = app.emit(
            "core://llm-download-progress",
            json!({ "progress": 1.0, "stage": "模型下载完成" }),
        );
        return Ok(destination.display().to_string());
    }
    Err(CoreError::new(
        "MODEL_DOWNLOAD_FAILED",
        "镜像源与官方源均无法继续下载语言模型，已保留断点文件供下次续传。",
    )
    .with_detail(last_error))
}

fn sha256_file(path: &Path) -> CoreResult<String> {
    let mut file = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 4 * 1024 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

enum DocumentProvider {
    MiniMax { api_key: String },
    Local,
}

fn normalize_document_type(value: Option<String>) -> CoreResult<String> {
    let value = value.unwrap_or_else(|| "meeting_notes".to_string());
    match value.trim() {
        "meeting_notes" | "executive_summary" | "action_plan" | "interview_insights" => {
            Ok(value.trim().to_string())
        }
        _ => Err(CoreError::new(
            "DOCUMENT_TYPE_UNSUPPORTED",
            "暂不支持该文档模板。",
        )),
    }
}

pub fn start(
    input: GenerateDocumentInput,
    app: AppHandle,
    state: &CoreState,
) -> CoreResult<TaskRecord> {
    let project = state
        .database
        .get_project(&input.project_id)?
        .ok_or_else(|| CoreError::new("PROJECT_NOT_FOUND", "找不到要生成文档的项目。"))?;
    let segments = state.database.list_transcript_segments(&input.project_id)?;
    if segments.is_empty() {
        return Err(CoreError::new(
            "EMPTY_TRANSCRIPT",
            "请先完成语音转写，再生成 AI 文档。",
        ));
    }
    let paths = resolve_paths(&app, state)?;
    let provider = match minimax_api_key()? {
        Some(api_key) => DocumentProvider::MiniMax { api_key },
        None => {
            if !paths.model.is_file() || paths.llama_server.is_none() {
                return Err(CoreError::new(
                    "AI_PROVIDER_NOT_READY",
                    "请在设置中保存新的 MiniMax API 密钥，或准备本地 Qwen 备用模型。",
                ));
            }
            DocumentProvider::Local
        }
    };
    let document_type = normalize_document_type(input.document_type)?;
    let reservation_id = format!("llm:{}", uuid::Uuid::new_v4());
    state.reserve_heavy_task(&reservation_id)?;
    let start_result = (|| -> CoreResult<TaskRecord> {
        let task = state.database.create_task(NewTaskInput {
            project_id: Some(input.project_id.clone()),
            kind: "llm".to_string(),
            input_json: Some(json!({ "documentType": document_type }).to_string()),
        })?;
        let running = state.database.transition_task(TransitionTaskInput {
            task_id: task.id.clone(),
            status: TaskStatus::Running,
            stage: Some(match &provider {
                DocumentProvider::MiniMax { .. } => "正在连接 MiniMax-M3".to_string(),
                DocumentProvider::Local => "正在启动本地 Qwen 备用模型".to_string(),
            }),
            progress: Some(0.0),
            error_code: None,
            error_message: None,
        })?;
        let _ = app.emit("core://task-updated", &running);
        let task_id = task.id;
        let project_id = input.project_id;
        let worker_reservation_id = reservation_id.clone();
        std::thread::spawn(move || {
            let worker_input = LlmWorkerInput {
                task_id: &task_id,
                project_id: &project_id,
                project_title: &project.title,
                document_type: &document_type,
                segments: &segments,
                paths,
                provider,
            };
            if let Err(error) = run_worker(worker_input, &app) {
                let state = app.state::<CoreState>();
                let current = state.database.get_task(&task_id).ok().flatten();
                if !matches!(
                    current.as_ref().map(|task| task.status),
                    Some(TaskStatus::Cancelled)
                ) {
                    if let Ok(failed) = state.database.transition_task(TransitionTaskInput {
                        task_id: task_id.clone(),
                        status: TaskStatus::Failed,
                        stage: Some("AI 文档生成失败".to_string()),
                        progress: current.as_ref().map(|task| task.progress),
                        error_code: Some(error.code.clone()),
                        error_message: Some(error.message.clone()),
                    }) {
                        let _ = app.emit("core://task-updated", &failed);
                    }
                    let _ = app.emit("core://document-error", &error);
                }
                if let Ok(mut processes) = state.llm_processes.lock() {
                    processes.remove(&task_id);
                };
            }
            app.state::<CoreState>()
                .release_heavy_task(&worker_reservation_id);
        });
        Ok(running)
    })();
    if start_result.is_err() {
        state.release_heavy_task(&reservation_id);
    }
    start_result
}

struct LlmWorkerInput<'a> {
    task_id: &'a str,
    project_id: &'a str,
    project_title: &'a str,
    document_type: &'a str,
    segments: &'a [crate::models::TranscriptSegment],
    paths: LlmPaths,
    provider: DocumentProvider,
}

fn run_worker(input: LlmWorkerInput<'_>, app: &AppHandle) -> CoreResult<()> {
    let LlmWorkerInput {
        task_id,
        project_id,
        project_title,
        document_type,
        segments,
        paths,
        provider,
    } = input;
    let mut child = sidecar_command(&paths)?;
    let mut child = child
        .args(["--model-dir"])
        .arg(&paths.asr_models)
        .env("PYTHONUTF8", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            CoreError::new("LLM_START_FAILED", "无法启动本地 AI 引擎。")
                .with_detail(error.to_string())
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CoreError::new("LLM_PIPE_FAILED", "无法读取本地 AI 引擎输出。"))?;
    let stderr = child.stderr.take();
    let mut request = json!({
        "type": "generate_document",
        "taskId": task_id,
        "projectTitle": project_title,
        "documentType": document_type,
        "contextSize": 12288,
        "segments": segments,
    });
    match provider {
        DocumentProvider::MiniMax { api_key } => {
            request["provider"] = json!("minimax");
            request["apiEndpoint"] = json!(MINIMAX_ENDPOINT);
            request["apiKey"] = json!(api_key);
            request["modelName"] = json!(MINIMAX_MODEL);
        }
        DocumentProvider::Local => {
            let llama_server = paths.llama_server.as_ref().ok_or_else(|| {
                CoreError::new("LLM_RUNTIME_NOT_FOUND", "找不到本地 Qwen 推理运行环境。")
            })?;
            request["provider"] = json!("local");
            request["llamaServer"] = json!(llama_server);
            request["modelPath"] = json!(paths.model);
            request["modelName"] = json!(MODEL_NAME);
        }
    }
    if let Some(mut stdin) = child.stdin.take() {
        writeln!(stdin, "{}", request).map_err(|error| {
            CoreError::new("LLM_REQUEST_FAILED", "无法向本地 AI 引擎发送任务。")
                .with_detail(error.to_string())
        })?;
    }
    let child = Arc::new(Mutex::new(child));
    app.state::<CoreState>()
        .llm_processes
        .lock()
        .map_err(|_| CoreError::new("LLM_PROCESS_LOCKED", "本地 AI 进程管理器暂时不可用。"))?
        .insert(task_id.to_string(), Arc::clone(&child));
    if matches!(
        app.state::<CoreState>()
            .database
            .get_task(task_id)?
            .map(|task| task.status),
        Some(TaskStatus::Cancelled)
    ) {
        if let Ok(mut child) = child.lock() {
            let _ = child.kill();
        }
        return Err(CoreError::new("TASK_CANCELLED", "AI 文档任务已取消。"));
    }
    let stderr_reader = stderr.map(|mut stream| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stream.read_to_string(&mut text);
            text
        })
    });

    let mut completed = false;
    let mut document_id: Option<String> = None;
    let mut engine_error: Option<CoreError> = None;
    for line in BufReader::new(stdout).lines() {
        let line = line?;
        let Ok(event) = serde_json::from_str::<Value>(&line) else {
            log::warn!("忽略无法解析的 LLM 输出: {}", line);
            continue;
        };
        match event["type"].as_str().unwrap_or_default() {
            "progress" => update_progress(
                app,
                task_id,
                event["progress"].as_f64().unwrap_or(0.0),
                event["stage"].as_str().unwrap_or("正在生成 AI 文档"),
            ),
            "document" => {
                let content = event.get("document").cloned().ok_or_else(|| {
                    CoreError::new("INVALID_LLM_DOCUMENT", "本地模型没有返回有效文档。")
                })?;
                let title = content["title"].as_str().unwrap_or(project_title);
                let state = app.state::<CoreState>();
                let document = state.database.create_ai_document(
                    project_id,
                    document_type,
                    title,
                    &content,
                    event["model"].as_str().unwrap_or(MODEL_NAME),
                    event["promptVersion"]
                        .as_str()
                        .unwrap_or("meeting-notes-v1"),
                )?;
                document_id = Some(document.id.clone());
                let _ = app.emit("core://document-created", &document);
            }
            "completed" => {
                let id = document_id.as_deref().ok_or_else(|| {
                    CoreError::new("LLM_DOCUMENT_MISSING", "本地模型完成任务但没有生成文档。")
                })?;
                let state = app.state::<CoreState>();
                let finished = state.database.complete_llm_task(task_id, id)?;
                let _ = app.emit("core://task-updated", &finished);
                completed = true;
            }
            "error" => {
                engine_error = Some(
                    CoreError::new(
                        event["code"].as_str().unwrap_or("LLM_ENGINE_ERROR"),
                        event["message"].as_str().unwrap_or("AI 文档生成失败。"),
                    )
                    .with_detail(event["detail"].as_str().unwrap_or_default()),
                );
            }
            _ => {}
        }
    }

    let status = child
        .lock()
        .map_err(|_| CoreError::new("LLM_PROCESS_LOCKED", "无法回收本地 AI 进程。"))?
        .wait()?;
    let stderr_text = stderr_reader
        .and_then(|thread| thread.join().ok())
        .unwrap_or_default();
    if let Ok(mut processes) = app.state::<CoreState>().llm_processes.lock() {
        processes.remove(task_id);
    }
    if let Some(error) = engine_error {
        return Err(error);
    }
    if !status.success() && !completed {
        return Err(
            CoreError::new("LLM_PROCESS_FAILED", "本地 AI 进程意外退出。")
                .with_detail(stderr_text.trim()),
        );
    }
    if !completed {
        return Err(CoreError::new("LLM_INCOMPLETE", "本地 AI 未返回完成状态。"));
    }
    Ok(())
}

fn update_progress(app: &AppHandle, task_id: &str, progress: f64, stage: &str) {
    let state = app.state::<CoreState>();
    if let Ok(task) = state
        .database
        .update_task_progress(task_id, progress, stage)
    {
        let _ = app.emit("core://task-updated", &task);
    }
}

pub fn cancel(task_id: &str, app: &AppHandle, state: &CoreState) -> CoreResult<TaskRecord> {
    if let Some(child) = state
        .llm_processes
        .lock()
        .map_err(|_| CoreError::new("LLM_PROCESS_LOCKED", "本地 AI 进程管理器暂时不可用。"))?
        .get(task_id)
        .cloned()
    {
        if let Ok(mut child) = child.lock() {
            #[cfg(windows)]
            {
                let pid = child.id().to_string();
                let _ = hidden_command(Path::new("taskkill.exe"))
                    .args(["/PID", &pid, "/T", "/F"])
                    .status();
            }
            let _ = child.kill();
        }
    }
    let task = state.database.transition_task(TransitionTaskInput {
        task_id: task_id.to_string(),
        status: TaskStatus::Cancelled,
        stage: Some("已取消 AI 生成".to_string()),
        progress: None,
        error_code: None,
        error_message: None,
    })?;
    let _ = app.emit("core://task-updated", &task);
    Ok(task)
}

#[cfg(test)]
mod tests {
    use super::{normalize_document_type, provider_status_without_state, sha256_file};

    #[test]
    fn hashes_model_files_with_sha256() {
        let path =
            std::env::temp_dir().join(format!("voicedesk-sha256-{}.txt", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"abc").expect("fixture is written");
        let actual = sha256_file(&path).expect("fixture is hashed");
        let _ = std::fs::remove_file(path);
        assert_eq!(
            actual,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn validates_document_templates() {
        assert_eq!(
            normalize_document_type(Some("action_plan".to_string())).expect("known template"),
            "action_plan"
        );
        assert!(normalize_document_type(Some("unknown".to_string())).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn reads_provider_status_from_windows_credential_manager() {
        let status = provider_status_without_state().expect("credential status can be read");
        assert_eq!(status.model, "MiniMax-M3");
        assert!(status.endpoint.starts_with("https://api.minimaxi.com/"));
    }
}
