use crate::bdmv::Track;
use crate::tool_error::format_tool_launch_error;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

// ============================================================================
// Muxer State
// ============================================================================

#[derive(Debug, Default)]
pub struct Muxer {
    pub is_running: bool,
    pub is_paused: bool,
    pub shutdown_requested: bool,
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
    pub resume_reason: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

// ============================================================================
// Muxer Implementation
// ============================================================================

impl Muxer {
    /// Run mkvmerge for a single job, applying track language/name edits.
    pub async fn run_mkvmerge(mkvmerge_path: &str, job: &MuxJob) -> Result<(), String> {
        let final_output_path = Path::new(&job.output_path);
        let staged_output_path = staged_output_path(final_output_path);
        if tokio::fs::try_exists(&staged_output_path)
            .await
            .map_err(|error| {
                format!(
                    "Failed to check for interrupted output {}: {}",
                    staged_output_path.display(),
                    error
                )
            })?
        {
            tokio::fs::remove_file(&staged_output_path)
                .await
                .map_err(|error| {
                    format!(
                        "Failed to remove interrupted output {}: {}",
                        staged_output_path.display(),
                        error
                    )
                })?;
        }

        let mut cmd = Command::new(mkvmerge_path);

        cmd.arg("-o").arg(&staged_output_path);

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

        let stderr = child.stderr.take().expect("stderr was piped");
        let stderr_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            let mut stderr = stderr;
            stderr.read_to_end(&mut bytes).await.map(|_| bytes)
        });

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

        let status = child
            .wait()
            .await
            .map_err(|e| format!("mkvmerge wait error: {}", e))?;
        let stderr = stderr_task
            .await
            .map_err(|error| format!("mkvmerge stderr reader failed: {}", error))?
            .map_err(|error| format!("Could not read mkvmerge diagnostics: {}", error))?;
        let stderr = String::from_utf8_lossy(&stderr);

        if status.success() || status.code() == Some(1) {
            // mkvmerge returns 1 for warnings, which is still a success.
            promote_staged_output(&staged_output_path, final_output_path).await
        } else {
            let _ = tokio::fs::remove_file(&staged_output_path).await;
            let diagnostics = stderr.trim();
            if diagnostics.is_empty() {
                Err(format!("mkvmerge exited with code {:?}", status.code()))
            } else {
                Err(format!(
                    "mkvmerge exited with code {:?}: {}",
                    status.code(),
                    diagnostics
                ))
            }
        }
    }
}

fn staged_output_path(output_path: &Path) -> PathBuf {
    let filename = output_path
        .file_name()
        .and_then(|filename| filename.to_str())
        .unwrap_or("output.mkv");
    output_path.with_file_name(format!(".{}.partial.mkv", filename))
}

async fn promote_staged_output(staged_path: &Path, output_path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    if tokio::fs::try_exists(output_path).await.unwrap_or(false) {
        tokio::fs::remove_file(output_path).await.map_err(|error| {
            format!(
                "Failed to replace output {}: {}",
                output_path.display(),
                error
            )
        })?;
    }

    tokio::fs::rename(staged_path, output_path)
        .await
        .map_err(|error| {
            format!(
                "Failed to finish output {}: {}",
                output_path.display(),
                error
            )
        })
}

#[cfg(test)]
mod tests {
    use super::{staged_output_path, MuxJob, Muxer};
    use crate::bdmv::Track;
    use std::fs;
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

    #[cfg(unix)]
    #[tokio::test]
    async fn successful_mux_replaces_stale_stage_and_promotes_complete_output() {
        use std::os::unix::fs::PermissionsExt;

        let test_dir = std::env::temp_dir().join(format!(
            "bdmv-remuxer-mux-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&test_dir).unwrap();
        let tool_path = test_dir.join("fake-mkvmerge");
        fs::write(
            &tool_path,
            "#!/bin/sh\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = \"-o\" ]; then shift; out=\"$1\"; fi\n  shift\ndone\nprintf 'complete output' > \"$out\"\n",
        )
        .unwrap();
        fs::set_permissions(&tool_path, fs::Permissions::from_mode(0o755)).unwrap();

        let final_path = test_dir.join("episode.mkv");
        fs::write(&final_path, "previous output").unwrap();
        let staged_path = staged_output_path(&final_path);
        fs::write(&staged_path, "interrupted output").unwrap();

        let job = MuxJob {
            input_path: "input.m2ts".into(),
            output_path: final_path.to_string_lossy().to_string(),
            chapter_path: None,
            tracks: Vec::<Track>::new(),
        };
        Muxer::run_mkvmerge(&tool_path.to_string_lossy(), &job)
            .await
            .unwrap();

        assert_eq!(fs::read_to_string(&final_path).unwrap(), "complete output");
        assert!(!staged_path.exists());

        fs::remove_dir_all(test_dir).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_mux_discards_staged_output_and_preserves_existing_output() {
        use std::os::unix::fs::PermissionsExt;

        let test_dir = std::env::temp_dir().join(format!(
            "bdmv-remuxer-mux-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&test_dir).unwrap();
        let tool_path = test_dir.join("fake-mkvmerge");
        fs::write(
            &tool_path,
            "#!/bin/sh\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = \"-o\" ]; then shift; out=\"$1\"; fi\n  shift\ndone\nprintf 'partial output' > \"$out\"\necho 'fixture media could not be read' >&2\nexit 2\n",
        )
        .unwrap();
        fs::set_permissions(&tool_path, fs::Permissions::from_mode(0o755)).unwrap();

        let final_path = test_dir.join("episode.mkv");
        fs::write(&final_path, "previous output").unwrap();
        let staged_path = staged_output_path(&final_path);
        let job = MuxJob {
            input_path: "input.m2ts".into(),
            output_path: final_path.to_string_lossy().to_string(),
            chapter_path: None,
            tracks: Vec::<Track>::new(),
        };

        let error = Muxer::run_mkvmerge(&tool_path.to_string_lossy(), &job)
            .await
            .unwrap_err();
        assert!(error.contains("fixture media could not be read"), "{error}");
        assert_eq!(fs::read_to_string(&final_path).unwrap(), "previous output");
        assert!(!staged_path.exists());

        fs::remove_dir_all(test_dir).unwrap();
    }
}
