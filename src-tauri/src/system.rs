use sysinfo::{Disks, System};

use crate::models::SystemSnapshot;

pub fn snapshot() -> SystemSnapshot {
    let mut system = System::new_all();
    system.refresh_all();
    let disks = Disks::new_with_refreshed_list();
    let available_disk_bytes = disks.iter().map(|disk| disk.available_space()).sum();
    SystemSnapshot {
        os_name: System::name().unwrap_or_else(|| "Windows".to_string()),
        os_version: System::os_version().unwrap_or_else(|| "未知版本".to_string()),
        cpu_name: system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_else(|| "未知处理器".to_string()),
        cpu_count: system.cpus().len(),
        total_memory_bytes: system.total_memory(),
        available_memory_bytes: system.available_memory(),
        available_disk_bytes,
        ffmpeg_available: crate::media::tool_available("VOICEDESK_FFMPEG", "ffmpeg"),
        ffprobe_available: crate::media::tool_available("VOICEDESK_FFPROBE", "ffprobe"),
    }
}
