use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;

use crate::{
    error::{CoreError, CoreResult},
    models::MediaProbe,
};

#[derive(Debug, Deserialize)]
struct ProbeOutput {
    #[serde(default)]
    streams: Vec<ProbeStream>,
    format: Option<ProbeFormat>,
}

#[derive(Debug, Deserialize)]
struct ProbeStream {
    codec_type: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ProbeFormat {
    format_name: Option<String>,
    duration: Option<String>,
    size: Option<String>,
}

fn executable(environment_key: &str, fallback: &str) -> String {
    std::env::var(environment_key).unwrap_or_else(|_| fallback.to_string())
}

fn cache_key(source_path: &str) -> CoreResult<u64> {
    let mut hasher = DefaultHasher::new();
    source_path.hash(&mut hasher);
    let metadata = std::fs::metadata(source_path)?;
    metadata.len().hash(&mut hasher);
    metadata.modified().ok().hash(&mut hasher);
    Ok(hasher.finish())
}

pub fn tool_available(environment_key: &str, fallback: &str) -> bool {
    Command::new(executable(environment_key, fallback))
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn probe_media(path: &str) -> CoreResult<MediaProbe> {
    let source = Path::new(path);
    if !source.is_file() {
        return Err(CoreError::new("MEDIA_NOT_FOUND", "找不到所选媒体文件。"));
    }
    let output = match Command::new(executable("VOICEDESK_FFPROBE", "ffprobe"))
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=format_name,duration,size:stream=codec_type,sample_rate,channels",
            "-of",
            "json",
            path,
        ])
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            log::warn!("未找到 ffprobe，使用基础文件信息继续导入: {}", path);
            return basic_probe(source);
        }
        Err(error) => return Err(error.into()),
    };

    if !output.status.success() {
        return Err(CoreError::new("MEDIA_PROBE_FAILED", "无法读取此媒体文件。")
            .with_detail(String::from_utf8_lossy(&output.stderr).trim().to_string()));
    }

    let parsed: ProbeOutput = serde_json::from_slice(&output.stdout)?;
    let audio = parsed
        .streams
        .iter()
        .find(|stream| stream.codec_type.as_deref() == Some("audio"));
    let has_video = parsed
        .streams
        .iter()
        .any(|stream| stream.codec_type.as_deref() == Some("video"));
    let metadata_size = std::fs::metadata(source)?.len() as i64;
    let format = parsed.format;
    let duration_ms = format
        .as_ref()
        .and_then(|value| value.duration.as_deref())
        .and_then(|value| value.parse::<f64>().ok())
        .map(|seconds| (seconds * 1000.0).round() as i64)
        .unwrap_or(0);
    let size_bytes = format
        .as_ref()
        .and_then(|value| value.size.as_deref())
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(metadata_size);

    Ok(MediaProbe {
        path: source.to_string_lossy().to_string(),
        display_name: source
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "未命名媒体".to_string()),
        format_name: format.and_then(|value| value.format_name),
        duration_ms,
        size_bytes,
        sample_rate: audio
            .and_then(|stream| stream.sample_rate.as_deref())
            .and_then(|value| value.parse().ok()),
        channels: audio.and_then(|stream| stream.channels),
        has_video,
    })
}

fn basic_probe(source: &Path) -> CoreResult<MediaProbe> {
    let extension = source
        .extension()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    const AUDIO: &[&str] = &["mp3", "wav", "m4a", "aac", "flac", "ogg", "opus", "wma"];
    const VIDEO: &[&str] = &["mp4", "mov", "mkv", "webm", "avi", "wmv"];
    if !AUDIO.contains(&extension.as_str()) && !VIDEO.contains(&extension.as_str()) {
        return Err(CoreError::new("UNSUPPORTED_MEDIA", "暂不支持此媒体格式。"));
    }
    Ok(MediaProbe {
        path: source.to_string_lossy().to_string(),
        display_name: source
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "未命名媒体".to_string()),
        format_name: Some(extension.clone()),
        duration_ms: 0,
        size_bytes: std::fs::metadata(source)?.len() as i64,
        sample_rate: None,
        channels: None,
        has_video: VIDEO.contains(&extension.as_str()),
    })
}

pub fn cache_waveform(source_path: &str, cache_directory: &Path) -> CoreResult<PathBuf> {
    if !Path::new(source_path).is_file() {
        return Err(CoreError::new("MEDIA_NOT_FOUND", "找不到所选媒体文件。"));
    }
    std::fs::create_dir_all(cache_directory)?;
    let target = cache_directory.join(format!("{:016x}.png", cache_key(source_path)?));
    if target.is_file() {
        return Ok(target);
    }

    let output = Command::new(executable("VOICEDESK_FFMPEG", "ffmpeg"))
        .args([
            "-y",
            "-v",
            "error",
            "-i",
            source_path,
            "-filter_complex",
            "aformat=channel_layouts=mono,showwavespic=s=1200x180:colors=#146EF5",
            "-frames:v",
            "1",
        ])
        .arg(&target)
        .output()
        .map_err(|error| {
            CoreError::new(
                "FFMPEG_NOT_FOUND",
                "尚未找到音频处理组件。请安装 FFmpeg，或在设置中指定工具位置。",
            )
            .with_detail(error.to_string())
        })?;
    if !output.status.success() {
        return Err(CoreError::new("WAVEFORM_FAILED", "无法生成音频波形缓存。")
            .with_detail(String::from_utf8_lossy(&output.stderr).trim().to_string()));
    }
    Ok(target)
}

pub fn transcode_to_pcm(source_path: &str, cache_directory: &Path) -> CoreResult<PathBuf> {
    if !Path::new(source_path).is_file() {
        return Err(CoreError::new("MEDIA_NOT_FOUND", "找不到所选媒体文件。"));
    }
    std::fs::create_dir_all(cache_directory)?;
    let target = cache_directory.join(format!("{:016x}.wav", cache_key(source_path)?));
    if target.is_file() {
        return Ok(target);
    }
    let partial = target.with_extension("wav.partial");
    let output = Command::new(executable("VOICEDESK_FFMPEG", "ffmpeg"))
        .args([
            "-y",
            "-v",
            "error",
            "-i",
            source_path,
            "-vn",
            "-ac",
            "1",
            "-ar",
            "16000",
            "-c:a",
            "pcm_s16le",
            "-f",
            "wav",
        ])
        .arg(&partial)
        .output()
        .map_err(|error| {
            CoreError::new(
                "FFMPEG_NOT_FOUND",
                "尚未找到音频处理组件。请安装 FFmpeg，或在设置中指定工具位置。",
            )
            .with_detail(error.to_string())
        })?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&partial);
        return Err(
            CoreError::new("TRANSCODE_FAILED", "无法准备语音识别所需的音频。")
                .with_detail(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        );
    }
    std::fs::rename(&partial, &target)?;
    Ok(target)
}

pub fn prepare_playback(source_path: &str, cache_directory: &Path) -> CoreResult<PathBuf> {
    let source = Path::new(source_path);
    if !source.is_file() {
        return Err(CoreError::new(
            "MEDIA_NOT_FOUND",
            "找不到此项目的原始媒体文件。",
        ));
    }
    let extension = source
        .extension()
        .map(|value| value.to_string_lossy().to_lowercase())
        .filter(|value| {
            value
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        })
        .unwrap_or_else(|| "media".to_string());
    std::fs::create_dir_all(cache_directory)?;
    let target = cache_directory.join(format!("{:016x}.{extension}", cache_key(source_path)?));
    let source_size = std::fs::metadata(source)?.len();
    if std::fs::metadata(&target)
        .map(|value| value.len() == source_size)
        .unwrap_or(false)
    {
        return Ok(target);
    }
    if target.exists() {
        std::fs::remove_file(&target)?;
    }
    if std::fs::hard_link(source, &target).is_err() {
        std::fs::copy(source, &target)?;
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::prepare_playback;

    #[test]
    fn prepares_media_inside_controlled_cache() {
        let root =
            std::env::temp_dir().join(format!("voicedesk-playback-{}", uuid::Uuid::new_v4()));
        let source = root.join("source.wav");
        let cache = root.join("cache");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&source, b"RIFF-test-audio").unwrap();
        let prepared = prepare_playback(source.to_str().unwrap(), &cache).unwrap();
        assert!(prepared.starts_with(&cache));
        assert_eq!(std::fs::read(prepared).unwrap(), b"RIFF-test-audio");
        let _ = std::fs::remove_dir_all(root);
    }
}
