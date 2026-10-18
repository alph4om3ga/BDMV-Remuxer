use crate::bdmv::{Track, Volume};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

// ============================================================================
// Settings
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub mkvmerge_path: String,
    pub ffprobe_path: String,
    /// Minimum duration in seconds for a file to be classified as an episode
    pub min_episode_duration: i64,
    /// Minimum duration in seconds for a file to be classified as an extra/special
    pub min_extra_duration: i64,
    /// Minimum duration in seconds for OP/ED detection range
    pub op_ed_min_duration: i64,
    /// Maximum duration in seconds for OP/ED detection range
    pub op_ed_max_duration: i64,
    pub episode_folder_name: String,
    pub extra_folder_name: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mkvmerge_path: "mkvmerge".into(),
            ffprobe_path: "ffprobe".into(),
            min_episode_duration: 1200, // 20 minutes
            min_extra_duration: 120,    // 2 minutes
            op_ed_min_duration: 80,
            op_ed_max_duration: 110,
            episode_folder_name: "Episode".into(),
            extra_folder_name: "Extra".into(),
        }
    }
}

impl Settings {
    const FILE_NAME: &'static str = "settings.json";

    pub fn load_from_config_dir(config_dir: &Path) -> Result<Self, String> {
        let path = config_dir.join(Self::FILE_NAME);
        match fs::read(&path) {
            Ok(contents) => serde_json::from_slice(&contents).map_err(|error| {
                format!("Failed to parse settings at {}: {}", path.display(), error)
            }),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!(
                "Failed to read settings at {}: {}",
                path.display(),
                error
            )),
        }
    }

    pub fn save_to_config_dir(&self, config_dir: &Path) -> Result<(), String> {
        fs::create_dir_all(config_dir).map_err(|error| {
            format!(
                "Failed to create settings directory {}: {}",
                config_dir.display(),
                error
            )
        })?;

        let path = config_dir.join(Self::FILE_NAME);
        let contents = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("Failed to serialize settings: {}", error))?;
        fs::write(&path, contents)
            .map_err(|error| format!("Failed to write settings at {}: {}", path.display(), error))
    }
}

// ============================================================================
// Project Status
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Pending,
    Processing,
    Paused,
    Complete,
    Error,
}

// ============================================================================
// Project
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub output_name: String,
    pub output_path: String,
    pub volumes: Vec<Volume>,
    pub track_templates: HashMap<String, Vec<Track>>,
    pub status: ProjectStatus,
    pub progress: f64,
    pub current_file: Option<String>,
    pub created_at: DateTime<Utc>,
    pub error: Option<String>,
}

// ============================================================================
// App State
// ============================================================================

#[derive(Debug, Default)]
pub struct AppState {
    pub current_volumes: Vec<Volume>,
    pub queue: Vec<Project>,
    pub settings: Settings,
    pub track_templates: HashMap<String, Vec<Track>>,
}

#[cfg(test)]
mod tests {
    use super::Settings;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_config_dir() -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "bdmv-remuxer-settings-test-{}-{}",
            std::process::id(),
            timestamp
        ))
    }

    #[test]
    fn missing_settings_file_uses_defaults() {
        let config_dir = temporary_config_dir();
        let settings = Settings::load_from_config_dir(&config_dir).unwrap();

        assert_eq!(settings.mkvmerge_path, Settings::default().mkvmerge_path);
        assert_eq!(
            settings.min_episode_duration,
            Settings::default().min_episode_duration
        );
    }

    #[test]
    fn settings_round_trip_through_json_file() {
        let config_dir = temporary_config_dir();
        let mut settings = Settings::default();
        settings.mkvmerge_path = "/custom/bin/mkvmerge".into();
        settings.ffprobe_path = "/custom/bin/ffprobe".into();
        settings.min_episode_duration = 1800;
        settings.op_ed_max_duration = 125;
        settings.episode_folder_name = "Episodes".into();

        settings.save_to_config_dir(&config_dir).unwrap();
        let loaded = Settings::load_from_config_dir(&config_dir).unwrap();

        assert_eq!(loaded.mkvmerge_path, settings.mkvmerge_path);
        assert_eq!(loaded.ffprobe_path, settings.ffprobe_path);
        assert_eq!(loaded.min_episode_duration, settings.min_episode_duration);
        assert_eq!(loaded.op_ed_max_duration, settings.op_ed_max_duration);
        assert_eq!(loaded.episode_folder_name, settings.episode_folder_name);

        std::fs::remove_dir_all(config_dir).unwrap();
    }
}
