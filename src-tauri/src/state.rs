use std::{
    collections::HashMap,
    path::PathBuf,
    process::Child,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc, Mutex,
    },
};

use crate::{
    database::Database,
    error::{CoreError, CoreResult},
};

pub struct CoreState {
    pub database: Database,
    pub app_data_directory: PathBuf,
    pub cache_directory: PathBuf,
    pub asr_processes: Mutex<HashMap<String, Arc<Mutex<Child>>>>,
    pub llm_processes: Mutex<HashMap<String, Arc<Mutex<Child>>>>,
    active_heavy_task: Mutex<Option<String>>,
    pub recordings: Mutex<HashMap<String, PathBuf>>,
    recovered_task_count: AtomicI64,
}

impl CoreState {
    pub fn new(
        database: Database,
        app_data_directory: PathBuf,
        cache_directory: PathBuf,
        recovered_task_count: i64,
    ) -> Self {
        Self {
            database,
            app_data_directory,
            cache_directory,
            asr_processes: Mutex::new(HashMap::new()),
            llm_processes: Mutex::new(HashMap::new()),
            active_heavy_task: Mutex::new(None),
            recordings: Mutex::new(HashMap::new()),
            recovered_task_count: AtomicI64::new(recovered_task_count),
        }
    }

    pub fn recovered_task_count(&self) -> i64 {
        self.recovered_task_count.load(Ordering::Relaxed)
    }

    pub fn reserve_heavy_task(&self, reservation_id: &str) -> CoreResult<()> {
        let mut active = self
            .active_heavy_task
            .lock()
            .map_err(|_| CoreError::new("HEAVY_TASK_LOCKED", "本地 AI 任务调度器暂时不可用。"))?;
        if active.is_some() {
            return Err(CoreError::new(
                "HEAVY_TASK_RUNNING",
                "已有语音识别或 AI 文档任务正在运行，请等待完成或先取消当前任务。",
            ));
        }
        *active = Some(reservation_id.to_string());
        Ok(())
    }

    pub fn release_heavy_task(&self, reservation_id: &str) {
        if let Ok(mut active) = self.active_heavy_task.lock() {
            if active.as_deref() == Some(reservation_id) {
                *active = None;
            }
        }
    }
}
