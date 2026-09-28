mod asr;
mod commands;
mod database;
mod error;
mod llm;
mod media;
mod models;
mod state;
mod system;
mod tasks;

use std::io;

use database::Database;
use log::LevelFilter;
use state::CoreState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(LevelFilter::Info)
                .level_for("reqwest", LevelFilter::Warn)
                .level_for("hyper", LevelFilter::Warn)
                .level_for("rustls", LevelFilter::Warn)
                .build(),
        )
        .setup(|app| {
            let app_data_directory = app.path().app_data_dir()?;
            let cache_directory = app.path().app_cache_dir()?;
            for directory in [
                app_data_directory.join("projects"),
                app_data_directory.join("models/asr"),
                app_data_directory.join("models/llm"),
                app_data_directory.join("recordings"),
                cache_directory.join("waveforms"),
                cache_directory.join("audio"),
            ] {
                std::fs::create_dir_all(directory)?;
            }
            let database = Database::open(app_data_directory.join("voicedesk.db"))
                .map_err(|error| io::Error::other(format!("{}: {}", error.code, error.message)))?;
            let recovered_task_count = database
                .restore_interrupted_tasks()
                .map_err(|error| io::Error::other(format!("{}: {}", error.code, error.message)))?;
            app.manage(CoreState::new(
                database,
                app_data_directory,
                cache_directory,
                recovered_task_count,
            ));
            log::info!(
                "VoiceDesk Core 已启动，恢复 {} 个任务",
                recovered_task_count
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::core_health,
            commands::list_projects,
            commands::list_deleted_projects,
            commands::trash_project,
            commands::restore_project,
            commands::create_project,
            commands::register_media,
            commands::probe_media,
            commands::cache_waveform,
            commands::prepare_audio,
            commands::prepare_project_playback,
            commands::enqueue_task,
            commands::list_tasks,
            commands::list_transcript_segments,
            commands::assign_speaker,
            commands::get_project_text,
            commands::save_project_text,
            commands::set_segment_review,
            commands::asr_health,
            commands::llm_health,
            commands::ai_provider_status,
            commands::save_minimax_api_key,
            commands::delete_minimax_api_key,
            commands::test_minimax_connection,
            commands::download_llm_model,
            commands::start_transcription,
            commands::cancel_transcription,
            commands::generate_document,
            commands::cancel_document_generation,
            commands::list_ai_documents,
            commands::begin_recording,
            commands::append_recording_chunk,
            commands::finish_recording,
            commands::cancel_recording,
            commands::update_transcript_segment,
            commands::replace_transcript_text,
            commands::export_transcript,
            commands::transition_task,
            commands::system_snapshot,
            commands::export_diagnostics,
        ])
        .run(tauri::generate_context!())
        .expect("VoiceDesk 启动失败");
}
