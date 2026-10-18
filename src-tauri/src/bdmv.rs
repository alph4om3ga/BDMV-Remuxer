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
        for file_path in &m2ts_files {
            match self.probe_file(file_path.to_string_lossy().as_ref()).await {
                Ok(media_file) => files.push(media_file),
                Err(e) => eprintln!("Warning: could not probe {:?}: {}", file_path, e),
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
        })
    }

    /// Probe a single media file with ffprobe
    pub async fn probe_file(&self, path: &str) -> Result<MediaFile, BdmvError> {
        use tokio::process::Command;

        let output = Command::new(&self.ffprobe_path)
            .args([
                "-v", "quiet",
                "-print_format", "json",
                "-show_streams",
                "-show_format",
                path,
            ])
            .output()
            .await
            .map_err(|e| BdmvError::Ffprobe(format!("Failed to run ffprobe: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BdmvError::Ffprobe(stderr.to_string()));
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
        const MIN_EPISODE: f64 = 1200.0; // 20 min
        const MIN_EXTRA: f64   = 120.0;  // 2 min
        const OP_ED_MIN: f64   = 80.0;
        const OP_ED_MAX: f64   = 110.0;

        let mut op_count = 0i32;
        let mut ed_count = 0i32;

        for file in files.iter_mut() {
            let d = file.duration;
            file.category = if d >= MIN_EPISODE {
                "EPISODE".into()
            } else if d >= OP_ED_MIN && d <= OP_ED_MAX {
                // Alternate OP/ED assignment by pairs
                let total = op_count + ed_count;
                if total % 2 == 0 {
                    op_count += 1;
                    "OP".into()
                } else {
                    ed_count += 1;
                    "ED".into()
                }
            } else if d >= MIN_EXTRA {
                "SPECIAL".into()
            } else {
                "OTHER".into()
            };
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
