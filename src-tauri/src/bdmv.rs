use crate::tool_error::format_tool_launch_error;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;
use walkdir::WalkDir;

// ============================================================================
// Error
// ============================================================================

#[derive(Debug, Error)]
pub enum BdmvError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("FFprobe error: {0}")]
    Ffprobe(String),
    #[error("{0}")]
    ToolLaunch(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("No BDMV/STREAM directory found in: {0}")]
    NoStream(String),
}

// ============================================================================
// Data Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: i32,
    pub track_type: String,
    pub codec: String,
    pub language: String,
    pub name: String,
    pub default: bool,
    pub forced: bool,
    pub resolution: Option<String>,
    pub channels: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFile {
    pub id: String,
    pub filename: String,
    pub path: String,
    pub duration: f64,
    pub size: u64,
    pub category: String,
    pub tracks: Vec<Track>,
    pub episode_number: Option<i32>,
    pub op_number: Option<i32>,
    pub ed_number: Option<i32>,
    pub special_number: Option<i32>,
    pub chapter_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub id: String,
    pub name: String,
    pub volume_number: usize,
    pub files: Vec<MediaFile>,
    #[serde(default)]
    pub probe_failures: Vec<ProbeFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeFailure {
    pub path: String,
    pub diagnostic: String,
}

// ============================================================================
// ffprobe JSON structures
// ============================================================================

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    streams: Vec<FfprobeStream>,
    format: FfprobeFormat,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    index: i32,
    codec_type: Option<String>,
    codec_name: Option<String>,
    #[serde(default)]
    tags: std::collections::HashMap<String, String>,
    #[serde(default)]
    disposition: std::collections::HashMap<String, i32>,
    // video-specific
    width: Option<i32>,
    height: Option<i32>,
    // audio-specific
    channels: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
    size: Option<String>,
    filename: Option<String>,
}

// ============================================================================
// Parser
// ============================================================================

pub struct BdmvParser {
    ffprobe_path: String,
}

impl BdmvParser {
    pub fn new() -> Self {
        Self {
            ffprobe_path: "ffprobe".into(),
        }
    }

    pub fn with_ffprobe(ffprobe_path: impl Into<String>) -> Self {
        Self {
            ffprobe_path: ffprobe_path.into(),
        }
    }

    /// Find all BDMV volumes (paths containing BDMV/STREAM) under a base path
    pub async fn find_bdmv_volumes(&self, base_path: &str) -> Result<Vec<String>, BdmvError> {
        let mut volumes = Vec::new();

        for entry in WalkDir::new(base_path)
            .max_depth(4)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_dir() && path.file_name().map(|n| n == "BDMV").unwrap_or(false) {
                if path.join("STREAM").exists() {
                    if let Some(parent) = path.parent() {
                        volumes.push(parent.to_string_lossy().to_string());
                    }
                }
            }
        }

        Ok(volumes)
    }

    /// Parse a single BDMV volume directory
    pub async fn parse_volume(&self, path: &str) -> Result<Volume, BdmvError> {
        let stream_path = Path::new(path).join("BDMV").join("STREAM");

        if !stream_path.exists() {
            return Err(BdmvError::NoStream(path.to_string()));
        }

        let volume_name = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown Volume".into());

        // Collect .m2ts files sorted by filename
        let mut m2ts_files: Vec<std::path::PathBuf> = std::fs::read_dir(&stream_path)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|ext| ext.eq_ignore_ascii_case("m2ts")).unwrap_or(false))
            .collect();

        m2ts_files.sort();

        // Parse each file with ffprobe
        let mut files: Vec<MediaFile> = Vec::new();
        let mut probe_failures = Vec::new();
        for file_path in &m2ts_files {
            match self.probe_file(file_path.to_string_lossy().as_ref()).await {
                Ok(media_file) => files.push(media_file),
                Err(error @ BdmvError::ToolLaunch(_)) => return Err(error),
                Err(error) => {
                    let diagnostic = error.to_string();
                    eprintln!("Warning: could not probe {:?}: {}", file_path, diagnostic);
                    probe_failures.push(ProbeFailure {
                        path: file_path.to_string_lossy().to_string(),
                        diagnostic,
                    });
                }
            }
        }

        // Auto-classify files
        self.classify_files(&mut files);

        // Auto-number
        self.auto_number(&mut files);

        // Look for chapter files
        let backup_path = Path::new(path).join("BDMV").join("BACKUP");
        for file in &mut files {
            let stem = Path::new(&file.filename).file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let chapter_candidate = backup_path.join(format!("{}.xml", stem));
            if chapter_candidate.exists() {
                file.chapter_file = Some(chapter_candidate.to_string_lossy().to_string());
            }
        }

        Ok(Volume {
            id: Uuid::new_v4().to_string(),
            name: volume_name,
            volume_number: 1,
            files,
            probe_failures,
        })
    }

    /// Probe a single media file with ffprobe
    pub async fn probe_file(&self, path: &str) -> Result<MediaFile, BdmvError> {
        use tokio::process::Command;

        let output = Command::new(&self.ffprobe_path)
            .args([
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_streams",
                "-show_format",
                path,
            ])
            .output()
            .await
            .map_err(|e| {
                BdmvError::ToolLaunch(format_tool_launch_error(
                    "ffprobe",
                    &self.ffprobe_path,
                    "FFprobe Path",
                    "FFmpeg",
                    e,
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let diagnostic = if stderr.trim().is_empty() {
                format!("ffprobe exited with status {}", output.status)
            } else {
                stderr.trim().to_string()
            };
            return Err(BdmvError::Ffprobe(diagnostic));
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let probe: FfprobeOutput = serde_json::from_str(&json_str)
            .map_err(|e| BdmvError::Parse(format!("JSON parse error: {}", e)))?;

        self.build_media_file(path, probe)
    }

    /// Get media info for an arbitrary file (not necessarily in a BDMV)
    pub async fn get_media_info(&self, path: &str) -> Result<MediaFile, BdmvError> {
        self.probe_file(path).await
    }

    /// Probe a previously skipped BDMV file and prepare it without changing
    /// metadata on files that have already been loaded or edited.
    pub async fn retry_file(
        &self,
        path: &str,
        existing_files: &[MediaFile],
        volume_path: &str,
    ) -> Result<MediaFile, BdmvError> {
        let mut file = self.probe_file(path).await?;
        let mut op_count = existing_files
            .iter()
            .filter(|f| f.category == "OP")
            .count() as i32;
        let mut ed_count = existing_files
            .iter()
            .filter(|f| f.category == "ED")
            .count() as i32;
        self.classify_file(&mut file, &mut op_count, &mut ed_count);
        self.assign_next_number(&mut file, existing_files);

        let stem = Path::new(&file.filename)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let chapter_candidate = Path::new(volume_path)
            .join("BDMV")
            .join("BACKUP")
            .join(format!("{}.xml", stem));
        if chapter_candidate.exists() {
            file.chapter_file = Some(chapter_candidate.to_string_lossy().to_string());
        }

        Ok(file)
    }

    fn build_media_file(&self, path: &str, probe: FfprobeOutput) -> Result<MediaFile, BdmvError> {
        let duration: f64 = probe
            .format
            .duration
            .as_deref()
            .and_then(|d| d.parse().ok())
            .unwrap_or(0.0);

        let size: u64 = probe
            .format
            .size
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| {
                std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
            });

        let filename = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());

        let mut tracks = Vec::new();
        for stream in &probe.streams {
            let track_type = match stream.codec_type.as_deref() {
                Some("video")    => "video",
                Some("audio")    => "audio",
                Some("subtitle") => "subtitle",
                _                => continue,
            };

            let language = stream
                .tags
                .get("language")
                .cloned()
                .unwrap_or_else(|| "und".into());

            let name = stream
                .tags
                .get("title")
                .cloned()
                .unwrap_or_default();

            let default = stream.disposition.get("default").copied().unwrap_or(0) != 0;
            let forced  = stream.disposition.get("forced").copied().unwrap_or(0) != 0;

            let resolution = if track_type == "video" {
                stream.width.zip(stream.height).map(|(w, h)| format!("{}x{}", w, h))
            } else {
                None
            };

            let channels = if track_type == "audio" {
                stream.channels.map(|c| format!("{} ch", c))
            } else {
                None
            };

            tracks.push(Track {
                id: stream.index,
                track_type: track_type.into(),
                codec: stream.codec_name.clone().unwrap_or_default(),
                language,
                name,
                default,
                forced,
                resolution,
                channels,
            });
        }

        Ok(MediaFile {
            id: Uuid::new_v4().to_string(),
            filename,
            path: path.to_string(),
            duration,
            size,
            category: "OTHER".into(),
            tracks,
            episode_number: None,
            op_number: None,
            ed_number: None,
            special_number: None,
            chapter_file: None,
        })
    }

    /// Auto-classify files by duration using default thresholds
    fn classify_files(&self, files: &mut Vec<MediaFile>) {
        let mut op_count = 0i32;
        let mut ed_count = 0i32;

        for file in files.iter_mut() {
            self.classify_file(file, &mut op_count, &mut ed_count);
        }
    }

    fn classify_file(&self, file: &mut MediaFile, op_count: &mut i32, ed_count: &mut i32) {
        const MIN_EPISODE: f64 = 1200.0; // 20 min
        const MIN_EXTRA: f64 = 120.0; // 2 min
        const OP_ED_MIN: f64 = 80.0;
        const OP_ED_MAX: f64 = 110.0;

        file.category = if file.duration >= MIN_EPISODE {
            "EPISODE".into()
        } else if file.duration >= OP_ED_MIN && file.duration <= OP_ED_MAX {
            // Alternate OP/ED assignment by pairs.
            if (*op_count + *ed_count) % 2 == 0 {
                *op_count += 1;
                "OP".into()
            } else {
                *ed_count += 1;
                "ED".into()
            }
        } else if file.duration >= MIN_EXTRA {
            "SPECIAL".into()
        } else {
            "OTHER".into()
        };
    }

    fn assign_next_number(&self, file: &mut MediaFile, existing_files: &[MediaFile]) {
        match file.category.as_str() {
            "EPISODE" => {
                file.episode_number = Some(
                    existing_files
                        .iter()
                        .filter_map(|f| f.episode_number)
                        .max()
                        .unwrap_or(0)
                        + 1,
                );
            }
            "OP" => {
                file.op_number = Some(
                    existing_files
                        .iter()
                        .filter_map(|f| f.op_number)
                        .max()
                        .unwrap_or(0)
                        + 1,
                );
            }
            "ED" => {
                file.ed_number = Some(
                    existing_files
                        .iter()
                        .filter_map(|f| f.ed_number)
                        .max()
                        .unwrap_or(0)
                        + 1,
                );
            }
            "SPECIAL" => {
                file.special_number = Some(
                    existing_files
                        .iter()
                        .filter_map(|f| f.special_number)
                        .max()
                        .unwrap_or(0)
                        + 1,
                );
            }
            _ => {}
        }
    }

    /// Auto-assign sequential numbers to episodes, OPs, EDs, and specials
    fn auto_number(&self, files: &mut Vec<MediaFile>) {
        let mut ep = 1i32;
        let mut op = 1i32;
        let mut ed = 1i32;
        let mut sp = 1i32;

        for file in files.iter_mut() {
            match file.category.as_str() {
                "EPISODE" => { file.episode_number = Some(ep); ep += 1; }
                "OP"      => { file.op_number      = Some(op); op += 1; }
                "ED"      => { file.ed_number      = Some(ed); ed += 1; }
                "SPECIAL" => { file.special_number = Some(sp); sp += 1; }
                _         => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BdmvError, BdmvParser, MediaFile};
    use std::fs;
    use std::path::PathBuf;

    fn missing_tool_path(tool: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "bdmv-remuxer-test-missing-{tool}-{}.exe",
            std::process::id()
        ))
    }

    #[tokio::test]
    async fn missing_ffprobe_path_returns_actionable_error() {
        let tool_path = missing_tool_path("ffprobe");
        assert!(!tool_path.exists());

        let error = BdmvParser::with_ffprobe(tool_path.to_string_lossy().to_string())
            .get_media_info("sample.m2ts")
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("ffprobe"), "{error}");
        assert!(error.contains("FFprobe Path"), "{error}");
        assert!(error.contains("Settings"), "{error}");
        assert!(error.contains("FFmpeg"), "{error}");
        assert!(
            error.contains(&tool_path.to_string_lossy().to_string()),
            "{error}"
        );
    }

    #[tokio::test]
    async fn parsing_a_volume_does_not_hide_a_missing_ffprobe_error() {
        let root =
            std::env::temp_dir().join(format!("bdmv-remuxer-test-volume-{}", std::process::id()));
        let stream = root.join("BDMV").join("STREAM");
        fs::create_dir_all(&stream).unwrap();
        fs::write(stream.join("00000.m2ts"), b"").unwrap();

        let tool_path = missing_tool_path("ffprobe");
        assert!(!tool_path.exists());
        let result = BdmvParser::with_ffprobe(tool_path.to_string_lossy().to_string())
            .parse_volume(root.to_string_lossy().as_ref())
            .await;

        let _ = fs::remove_dir_all(&root);
        let error = result.unwrap_err();
        assert!(matches!(&error, BdmvError::ToolLaunch(_)));
        let message = error.to_string();
        assert!(message.contains("ffprobe"), "{message}");
        assert!(message.contains("FFprobe Path"), "{message}");
        assert!(message.contains("Settings"), "{message}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn parsing_keeps_good_files_and_reports_each_failed_probe() {
        use std::os::unix::fs::PermissionsExt;

        let root =
            std::env::temp_dir().join(format!("bdmv-remuxer-partial-probe-{}", std::process::id()));
        let stream = root.join("BDMV").join("STREAM");
        fs::create_dir_all(&stream).unwrap();
        fs::write(stream.join("00000-bad.m2ts"), b"bad").unwrap();
        fs::write(stream.join("00001-good.m2ts"), b"good").unwrap();

        let ffprobe = root.join("fake-ffprobe");
        fs::write(
            &ffprobe,
            r#"#!/bin/sh
for arg in "$@"; do input="$arg"; done
case "$input" in
  *bad.m2ts) echo "Invalid data found when processing input" >&2; exit 1 ;;
  *) echo '{"streams":[],"format":{"duration":"1800","size":"4"}}' ;;
esac
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&ffprobe).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&ffprobe, permissions).unwrap();

        let result = BdmvParser::with_ffprobe(ffprobe.to_string_lossy().to_string())
            .parse_volume(root.to_string_lossy().as_ref())
            .await
            .unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].filename.contains("good"));
        assert_eq!(result.probe_failures.len(), 1);
        assert!(result.probe_failures[0].path.ends_with("00000-bad.m2ts"));
        assert!(
            result.probe_failures[0]
                .diagnostic
                .contains("Invalid data found when processing input"),
            "{}",
            result.probe_failures[0].diagnostic
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn retrying_a_skipped_file_preserves_existing_metadata_and_assigns_its_number() {
        use std::os::unix::fs::PermissionsExt;

        let root =
            std::env::temp_dir().join(format!("bdmv-remuxer-retry-probe-{}", std::process::id()));
        let stream = root.join("BDMV").join("STREAM");
        let backup = root.join("BDMV").join("BACKUP");
        fs::create_dir_all(&stream).unwrap();
        fs::create_dir_all(&backup).unwrap();
        let retry_path = stream.join("00002-retry.m2ts");
        fs::write(&retry_path, b"good").unwrap();
        let chapter_path = backup.join("00002-retry.xml");
        fs::write(&chapter_path, b"<chapters/>").unwrap();

        let ffprobe = root.join("fake-ffprobe");
        fs::write(
            &ffprobe,
            r#"#!/bin/sh
echo '{"streams":[],"format":{"duration":"1800","size":"4"}}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&ffprobe).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&ffprobe, permissions).unwrap();

        let existing = MediaFile {
            id: "existing-file".into(),
            filename: "00001.m2ts".into(),
            path: stream.join("00001.m2ts").to_string_lossy().to_string(),
            duration: 1800.0,
            size: 4,
            category: "EPISODE".into(),
            tracks: Vec::new(),
            episode_number: Some(7),
            op_number: None,
            ed_number: None,
            special_number: None,
            chapter_file: None,
        };
        let result = BdmvParser::with_ffprobe(ffprobe.to_string_lossy().to_string())
            .retry_file(
                retry_path.to_string_lossy().as_ref(),
                &[existing.clone()],
                root.to_string_lossy().as_ref(),
            )
            .await
            .unwrap();

        assert_eq!(result.category, "EPISODE");
        assert_eq!(result.episode_number, Some(8));
        assert_eq!(
            result.chapter_file.as_deref(),
            Some(chapter_path.to_string_lossy().as_ref())
        );
        assert_eq!(existing.episode_number, Some(7));

        let _ = fs::remove_dir_all(&root);
    }
}
