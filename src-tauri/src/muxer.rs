use crate::bdmv::Track;
use crate::tool_error::format_tool_launch_error;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

// ============================================================================
// Muxer State
// ============================================================================

#[derive(Debug, Default)]
pub struct Muxer {
    pub is_running: bool,
    pub is_paused: bool,
}

// ============================================================================
// Mux Job
// ============================================================================

#[derive(Debug, Clone)]
pub struct MuxJob {
    pub input_path: String,
    pub output_path: String,
    pub chapter_path: Option<String>,
    pub tracks: Vec<Track>,
}

// ============================================================================
// Mux Progress (emitted as Tauri event)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MuxProgress {
    pub project_id: String,
    pub progress: f64,
    pub current_file: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

// ============================================================================
// Muxer Implementation
// ============================================================================

impl Muxer {
    /// Run mkvmerge for a single job, applying track language/name edits.
    pub async fn run_mkvmerge(mkvmerge_path: &str, job: &MuxJob) -> Result<(), String> {
        let mut cmd = Command::new(mkvmerge_path);

        cmd.arg("-o").arg(&job.output_path);

        // Build track-order and language/name arguments
        // First pass: collect audio/subtitle track edits
        let mut track_args: Vec<String> = Vec::new();
        let mut video_tracks: Vec<&Track> = Vec::new();
        let mut audio_tracks: Vec<&Track> = Vec::new();
        let mut subtitle_tracks: Vec<&Track> = Vec::new();

        for track in &job.tracks {
            match track.track_type.as_str() {
                "video"    => video_tracks.push(track),
                "audio"    => audio_tracks.push(track),
                "subtitle" => subtitle_tracks.push(track),
                _ => {}
            }
        }

        // Language flags per track type
        if !audio_tracks.is_empty() {
            let ids: Vec<String> = audio_tracks.iter().map(|t| t.id.to_string()).collect();
            let langs: Vec<String> = audio_tracks.iter().map(|t| format!("{}:{}", t.id, t.language)).collect();
            let names: Vec<String> = audio_tracks.iter()
                .filter(|t| !t.name.is_empty())
                .map(|t| format!("{}:{}", t.id, t.name))
                .collect();

            track_args.push("--audio-tracks".into());
            track_args.push(ids.join(","));
            for lang in &langs {
                track_args.push("--language".into());
                track_args.push(lang.clone());
            }
            for name in &names {
                track_args.push("--track-name".into());
                track_args.push(name.clone());
            }
        }

        if !subtitle_tracks.is_empty() {
            let ids: Vec<String> = subtitle_tracks.iter().map(|t| t.id.to_string()).collect();
            let langs: Vec<String> = subtitle_tracks.iter().map(|t| format!("{}:{}", t.id, t.language)).collect();
            let names: Vec<String> = subtitle_tracks.iter()
                .filter(|t| !t.name.is_empty())
                .map(|t| format!("{}:{}", t.id, t.name))
                .collect();

            track_args.push("--subtitle-tracks".into());
            track_args.push(ids.join(","));
            for lang in &langs {
                track_args.push("--language".into());
                track_args.push(lang.clone());
            }
            for name in &names {
                track_args.push("--track-name".into());
                track_args.push(name.clone());
            }
        }

        for arg in &track_args {
            cmd.arg(arg);
        }

        // Optional chapter file
        if let Some(chapter) = &job.chapter_path {
            cmd.arg("--chapters").arg(chapter);
        }

        cmd.arg(&job.input_path);

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            format_tool_launch_error(
                "mkvmerge",
                mkvmerge_path,
                "MKVToolNix Path",
                "MKVToolNix",
                e,
            )
        })?;

        if let Some(stdout) = child.stdout.take() {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                // Progress lines from mkvmerge look like: "Progress: 42%"
                if line.starts_with("Progress:") {
                    // Could emit finer progress here if needed
                    let _ = line;
                }
            }
        }

        let status = child.wait().await.map_err(|e| format!("mkvmerge wait error: {}", e))?;

        if status.success() || status.code() == Some(1) {
            // mkvmerge returns 1 for warnings, which is still a success
            Ok(())
        } else {
            Err(format!("mkvmerge exited with code {:?}", status.code()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MuxJob, Muxer};
    use crate::bdmv::Track;
    use std::path::PathBuf;

    #[tokio::test]
    async fn missing_mkvmerge_path_returns_actionable_error() {
        let tool_path: PathBuf = std::env::temp_dir().join(format!(
            "bdmv-remuxer-test-missing-mkvmerge-{}.exe",
            std::process::id()
        ));
        assert!(!tool_path.exists());
        let path = tool_path.to_string_lossy().to_string();
        let job = MuxJob {
            input_path: "input.m2ts".into(),
            output_path: "output.mkv".into(),
            chapter_path: None,
            tracks: Vec::<Track>::new(),
        };

        let error = Muxer::run_mkvmerge(&path, &job).await.unwrap_err();

        assert!(error.contains("mkvmerge"), "{error}");
        assert!(error.contains("MKVToolNix Path"), "{error}");
        assert!(error.contains("Settings"), "{error}");
        assert!(error.contains("MKVToolNix"), "{error}");
        assert!(error.contains(&path), "{error}");
    }
}
