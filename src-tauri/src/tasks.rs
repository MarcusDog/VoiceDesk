use crate::{
    error::{CoreError, CoreResult},
    models::TaskStatus,
};

pub fn validate_transition(from: TaskStatus, to: TaskStatus) -> CoreResult<()> {
    let allowed = matches!(
        (from, to),
        (
            TaskStatus::Queued,
            TaskStatus::Running | TaskStatus::Cancelled
        ) | (
            TaskStatus::Running,
            TaskStatus::Paused | TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) | (
            TaskStatus::Paused,
            TaskStatus::Running | TaskStatus::Cancelled
        ) | (
            TaskStatus::Failed,
            TaskStatus::Queued | TaskStatus::Cancelled
        )
    );
    if allowed {
        Ok(())
    } else {
        Err(CoreError::new(
            "INVALID_TASK_TRANSITION",
            format!("任务不能从 {} 切换为 {}。", from.as_str(), to.as_str()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_pause_and_resume() {
        assert!(validate_transition(TaskStatus::Running, TaskStatus::Paused).is_ok());
        assert!(validate_transition(TaskStatus::Paused, TaskStatus::Running).is_ok());
    }

    #[test]
    fn rejects_completed_to_running() {
        assert!(validate_transition(TaskStatus::Completed, TaskStatus::Running).is_err());
    }
}
