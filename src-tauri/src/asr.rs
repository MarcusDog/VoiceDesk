use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    error::{CoreError, CoreResult},
    models::{AsrHealth, NewTranscriptSegment, TaskRecord, TaskStatus, TransitionTaskInput},
    state::CoreState,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptionConfig {
    #[serde(default)]
    media_paths: Vec<String>,
    #[serde(default = "default_model")]
    model: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default = "default_device")]
    device: String,
    #[serde(default = "default_compute_type")]
    compute_type: String,
    #[serde(default = "default_beam_size")]
    beam_size: u8,
    #[serde(default)]
    initial_prompt: Option<String>,
}

fn default_model() -> String {
    "small".to_string()
}

fn default_device() -> String {
    "auto".to_string()
}

fn default_compute_type() -> String {
    "auto".to_string()
}

fn default_beam_size() -> u8 {
    5
}

#[derive(Debug)]
struct AsrPaths {
    binary: Option<PathBuf>,
    python: Option<PathBuf>,
    script: Option<PathBuf>,
    models: PathBuf,
}

fn first_file(paths: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    paths.into_iter().find(|path| path.is_file())
}

fn resolve_paths(app: &AppHandle, state: &CoreState) -> CoreResult<AsrPaths> {
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")));
    let resource_root = app.path().resource_dir().ok();
    let binary = std::env::var_os("VOICEDESK_ASR_BINARY")
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
    if let Some(binary) = binary {
        return Ok(AsrPaths {
            binary: Some(binary),
            python: None,
            script: None,
            models: state.app_data_directory.join("models/asr"),
        });
    }
    let python = std::env::var_os("VOICEDESK_ASR_PYTHON")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| {
            first_file([
                manifest_root.join(".venv-asr/Scripts/python.exe"),
                manifest_root.join(".venv-asr/bin/python"),
                state.app_data_directory.join("runtime/python/python.exe"),
            ])
        })
        .ok_or_else(|| {
            CoreError::new(
                "ASR_RUNTIME_NOT_FOUND",
                "尚未安装本地语音识别运行环境，请先执行 scripts/setup-asr.ps1。",
            )
        })?;
    let script = std::env::var_os("VOICEDESK_ASR_SCRIPT")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| {
            let mut candidates = vec![manifest_root.join("engines/asr/server.py")];
            if let Some(root) = resource_root {
                candidates.push(root.join("engines/asr/server.py"));
                candidates.push(root.join("_up_/engines/asr/server.py"));
            }
            first_file(candidates)
        })
        .ok_or_else(|| CoreError::new("ASR_SCRIPT_NOT_FOUND", "找不到本地语音识别引擎。"))?;
    Ok(AsrPaths {
        binary: None,
        python: Some(python),
        script: Some(script),
        models: state.app_data_directory.join("models/asr"),
    })
}

fn sidecar_command(paths: &AsrPaths) -> CoreResult<Command> {
    if let Some(binary) = paths.binary.as_ref() {
        return Ok(hidden_command(binary));
    }
    let python = paths
        .python
        .as_ref()
        .ok_or_else(|| CoreError::new("ASR_RUNTIME_NOT_FOUND", "找不到语音识别运行程序。"))?;
    let script = paths
        .script
        .as_ref()
        .ok_or_else(|| CoreError::new("ASR_SCRIPT_NOT_FOUND", "找不到本地语音识别引擎。"))?;
    let mut command = hidden_command(python);
    command.arg(script);
    Ok(command)
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

pub fn health(app: &AppHandle, state: &CoreState) -> AsrHealth {
    let model_directory = state.app_data_directory.join("models/asr");
    let paths = match resolve_paths(app, state) {
        Ok(paths) => paths,
        Err(error) => {
            return AsrHealth {
                available: false,
                python_path: None,
                script_path: None,
                engine_installed: false,
                engine_version: None,
                python_version: None,
                cuda_device_count: 0,
                model_directory: model_directory.display().to_string(),
                detail: Some(error.message),
            }
        }
    };
    let output = sidecar_command(&paths).and_then(|mut command| {
        command
            .args(["--health", "--model-dir"])
            .arg(&paths.models)
            .env("PYTHONUTF8", "1")
            .env("HF_ENDPOINT", "https://hf-mirror.com")
            .output()
            .map_err(Into::into)
    });
    match output {
        Ok(output) if output.status.success() => {
            let value = String::from_utf8_lossy(&output.stdout)
                .lines()
                .last()
                .and_then(|line| serde_json::from_str::<Value>(line).ok())
                .unwrap_or_default();
            AsrHealth {
                available: value["engineInstalled"].as_bool().unwrap_or(false),
                python_path: paths
                    .python
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .or_else(|| paths.binary.as_ref().map(|path| path.display().to_string())),
                script_path: paths.script.as_ref().map(|path| path.display().to_string()),
                engine_installed: value["engineInstalled"].as_bool().unwrap_or(false),
                engine_version: value["engineVersion"].as_str().map(str::to_string),
                python_version: value["python"].as_str().map(str::to_string),
                cuda_device_count: value["cudaDeviceCount"].as_i64().unwrap_or(0),
                model_directory: paths.models.display().to_string(),
                detail: None,
            }
        }
        Ok(output) => AsrHealth {
            available: false,
            python_path: paths
                .python
                .as_ref()
                .map(|path| path.display().to_string())
                .or_else(|| paths.binary.as_ref().map(|path| path.display().to_string())),
            script_path: paths.script.as_ref().map(|path| path.display().to_string()),
            engine_installed: false,
            engine_version: None,
            python_version: None,
            cuda_device_count: 0,
            model_directory: paths.models.display().to_string(),
            detail: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        },
        Err(error) => AsrHealth {
            available: false,
            python_path: paths
                .python
                .as_ref()
                .map(|path| path.display().to_string())
                .or_else(|| paths.binary.as_ref().map(|path| path.display().to_string())),
            script_path: paths.script.as_ref().map(|path| path.display().to_string()),
            engine_installed: false,
            engine_version: None,
            python_version: None,
            cuda_device_count: 0,
            model_directory: paths.models.display().to_string(),
            detail: Some(error.detail.unwrap_or(error.message)),
        },
    }
}

pub fn start(task_id: String, app: AppHandle, state: &CoreState) -> CoreResult<TaskRecord> {
    let task = state
        .database
        .get_task(&task_id)?
        .ok_or_else(|| CoreError::new("TASK_NOT_FOUND", "找不到对应任务。"))?;
    if task.kind != "transcription" {
        return Err(CoreError::new("INVALID_TASK_KIND", "该任务不是转写任务。"));
    }
    let project_id = task
        .project_id
        .clone()
        .ok_or_else(|| CoreError::new("PROJECT_REQUIRED", "转写任务必须关联项目。"))?;
    let config: TranscriptionConfig = serde_json::from_str(&task.input_json).map_err(|error| {
        CoreError::new("INVALID_TRANSCRIPTION_CONFIG", "转写任务参数无效。")
            .with_detail(error.to_string())
    })?;
    if config.media_paths.is_empty() {
        return Err(CoreError::new(
            "MEDIA_REQUIRED",
            "转写任务没有可处理的媒体文件。",
        ));
    }
    let paths = resolve_paths(&app, state)?;
    let reservation_id = format!("asr:{task_id}");
    state.reserve_heavy_task(&reservation_id)?;
    let start_result = (|| -> CoreResult<TaskRecord> {
        {
            let processes = state.asr_processes.lock().map_err(|_| {
                CoreError::new("ASR_PROCESS_LOCKED", "语音识别进程管理器暂时不可用。")
            })?;
            if processes.contains_key(&task_id) {
                return Err(CoreError::new(
                    "TASK_ALREADY_RUNNING",
                    "该转写任务正在运行。",
                ));
            }
        }

        if task.status == TaskStatus::Failed {
            state.database.transition_task(TransitionTaskInput {
                task_id: task_id.clone(),
                status: TaskStatus::Queued,
                stage: Some("等待重试".to_string()),
                progress: Some(0.0),
                error_code: None,
                error_message: None,
            })?;
        }
        let running = state.database.transition_task(TransitionTaskInput {
            task_id: task_id.clone(),
            status: TaskStatus::Running,
            stage: Some("正在启动本地识别引擎".to_string()),
            progress: Some(0.0),
            error_code: None,
            error_message: None,
        })?;
        state.database.begin_transcription(&project_id, &task_id)?;
        let _ = app.emit("core://task-updated", &running);

        let worker_reservation_id = reservation_id.clone();
        std::thread::spawn(move || {
            if let Err(error) = run_worker(&task_id, &project_id, config, paths, &app) {
                let state = app.state::<CoreState>();
                let current = state.database.get_task(&task_id).ok().flatten();
                if !matches!(
                    current.as_ref().map(|task| task.status),
                    Some(TaskStatus::Cancelled)
                ) {
                    if let Ok(failed) = state.database.fail_transcription(
                        &task_id,
                        &project_id,
                        &error.code,
                        &error.message,
                    ) {
                        let _ = app.emit("core://task-updated", &failed);
                    }
                    let _ = app.emit("core://transcription-error", &error);
                }
                if let Ok(mut processes) = state.asr_processes.lock() {
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

fn run_worker(
    task_id: &str,
    project_id: &str,
    config: TranscriptionConfig,
    paths: AsrPaths,
    app: &AppHandle,
) -> CoreResult<()> {
    let mut child = sidecar_command(&paths)?;
    let mut child = child
        .args(["--model-dir"])
        .arg(&paths.models)
        .env("PYTHONUTF8", "1")
        .env("HF_ENDPOINT", "https://hf-mirror.com")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            CoreError::new("ASR_START_FAILED", "无法启动本地语音识别引擎。")
                .with_detail(error.to_string())
        })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CoreError::new("ASR_PIPE_FAILED", "无法读取语音识别引擎输出。"))?;
    let stderr = child.stderr.take();
    let request = json!({
        "type": "transcribe",
        "taskId": task_id,
        "mediaPaths": config.media_paths,
        "model": config.model,
        "language": config.language,
        "device": config.device,
        "computeType": config.compute_type,
        "beamSize": config.beam_size,
        "initialPrompt": config.initial_prompt,
    });
    if let Some(mut stdin) = child.stdin.take() {
        writeln!(stdin, "{}", request).map_err(|error| {
            CoreError::new("ASR_REQUEST_FAILED", "无法向语音识别引擎发送任务。")
                .with_detail(error.to_string())
        })?;
    }
    let child = Arc::new(Mutex::new(child));
    {
        let state = app.state::<CoreState>();
        state
            .asr_processes
            .lock()
            .map_err(|_| CoreError::new("ASR_PROCESS_LOCKED", "语音识别进程管理器暂时不可用。"))?
            .insert(task_id.to_string(), Arc::clone(&child));
    }
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
        return Err(CoreError::new("TASK_CANCELLED", "转写任务已取消。"));
    }
    let stderr_reader = stderr.map(|mut stream| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stream.read_to_string(&mut text);
            text
        })
    });

    let mut completed = false;
    let mut engine_error: Option<CoreError> = None;
    for line in BufReader::new(stdout).lines() {
        let line = line?;
        let Ok(event) = serde_json::from_str::<Value>(&line) else {
            log::warn!("忽略无法解析的 ASR 输出: {}", line);
            continue;
        };
        match event["type"].as_str().unwrap_or_default() {
            "model_loading" => {
                let stage = if event["cached"].as_bool().unwrap_or(false) {
                    "正在加载本机识别模型"
                } else {
                    "正在下载识别模型（仅首次使用）"
                };
                update_progress(app, task_id, 0.01, stage);
            }
            "runtime_fallback" => {
                update_progress(app, task_id, 0.01, "GPU 不可用，已切换 CPU 模式");
                let _ = app.emit("core://asr-runtime-fallback", &event);
            }
            "media_started" => update_progress(app, task_id, 0.02, "正在分析音频并检测语种"),
            "progress" => {
                update_progress(
                    app,
                    task_id,
                    event["progress"].as_f64().unwrap_or(0.0),
                    event["stage"].as_str().unwrap_or("正在识别语音"),
                );
            }
            "segment" => {
                let state = app.state::<CoreState>();
                let segment = state
                    .database
                    .append_transcript_segment(NewTranscriptSegment {
                        project_id: project_id.to_string(),
                        task_id: task_id.to_string(),
                        sequence: event["sequence"].as_i64().unwrap_or(0),
                        start_ms: event["startMs"].as_i64().unwrap_or(0),
                        end_ms: event["endMs"].as_i64().unwrap_or(0),
                        text: event["text"].as_str().unwrap_or_default().to_string(),
                        confidence: event["confidence"].as_f64(),
                        language: event["language"].as_str().map(str::to_string),
                        model: event["model"].as_str().map(str::to_string),
                        chunk_id: event["chunkId"].as_str().map(str::to_string),
                    })?;
                let _ = app.emit("core://transcript-segment", &segment);
            }
            "completed" => {
                let state = app.state::<CoreState>();
                let finished = state.database.complete_transcription(
                    task_id,
                    project_id,
                    &event.to_string(),
                    event["durationMs"].as_i64().unwrap_or(0),
                    event["language"].as_str(),
                )?;
                let _ = app.emit("core://task-updated", &finished);
                let _ = app.emit("core://transcription-completed", &event);
                completed = true;
            }
            "error" => {
                engine_error = Some(
                    CoreError::new(
                        event["code"].as_str().unwrap_or("ASR_ENGINE_ERROR"),
                        event["message"].as_str().unwrap_or("语音识别失败。"),
                    )
                    .with_detail(event["detail"].as_str().unwrap_or_default()),
                );
            }
            _ => {}
        }
    }

    let status = child
        .lock()
        .map_err(|_| CoreError::new("ASR_PROCESS_LOCKED", "无法回收语音识别进程。"))?
        .wait()?;
    let stderr_text = stderr_reader
        .and_then(|thread| thread.join().ok())
        .unwrap_or_default();
    let state = app.state::<CoreState>();
    if let Ok(mut processes) = state.asr_processes.lock() {
        processes.remove(task_id);
    };
    if let Some(error) = engine_error {
        return Err(error);
    }
    if !status.success() && !completed {
        return Err(
            CoreError::new("ASR_PROCESS_FAILED", "语音识别进程意外退出。")
                .with_detail(stderr_text.trim()),
        );
    }
    if !completed {
        return Err(CoreError::new("ASR_INCOMPLETE", "语音识别未返回完成状态。"));
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
        .asr_processes
        .lock()
        .map_err(|_| CoreError::new("ASR_PROCESS_LOCKED", "语音识别进程管理器暂时不可用。"))?
        .get(task_id)
        .cloned()
    {
        let _ = child.lock().map(|mut child| child.kill());
    }
    let task = state.database.transition_task(TransitionTaskInput {
        task_id: task_id.to_string(),
        status: TaskStatus::Cancelled,
        stage: Some("已取消".to_string()),
        progress: None,
        error_code: None,
        error_message: None,
    })?;
    let _ = app.emit("core://task-updated", &task);
    Ok(task)
}
