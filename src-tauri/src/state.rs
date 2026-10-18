use crate::bdmv::{Track, Volume};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

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
// Persisted workspace
// ============================================================================

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct WorkspaceData {
    current_volumes: Vec<Volume>,
    queue: Vec<Project>,
    track_templates: HashMap<String, Vec<Track>>,
}

impl WorkspaceData {
    const FILE_NAME: &'static str = "workspace.json";

    fn load(config_dir: &Path) -> Result<Self, String> {
        let path = config_dir.join(Self::FILE_NAME);
        match fs::read(&path) {
            Ok(contents) => serde_json::from_slice(&contents).map_err(|error| {
                format!(
                    "Failed to parse saved work at {}: {}",
                    path.display(),
                    error
                )
            }),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!(
                "Failed to read saved work at {}: {}",
                path.display(),
                error
            )),
        }
    }

    fn save(&self, config_dir: &Path) -> Result<(), String> {
        fs::create_dir_all(config_dir).map_err(|error| {
            format!(
                "Failed to create workspace directory {}: {}",
                config_dir.display(),
                error
            )
        })?;

        let path = config_dir.join(Self::FILE_NAME);
        let temporary_path = config_dir.join(format!("{}.tmp", Self::FILE_NAME));
        let contents = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("Failed to serialize saved work: {}", error))?;
        fs::write(&temporary_path, contents).map_err(|error| {
            format!(
                "Failed to write saved work at {}: {}",
                temporary_path.display(),
                error
            )
        })?;
        fs::rename(&temporary_path, &path).map_err(|error| {
            format!(
                "Failed to replace saved work at {}: {}",
                path.display(),
                error
            )
        })
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
    persistence_path: Option<PathBuf>,
    persistence_error: Option<String>,
}

impl AppState {
    pub fn restore_workspace(&mut self, config_dir: &Path) -> Result<(), String> {
        let workspace = WorkspaceData::load(config_dir)?;
        self.current_volumes = workspace.current_volumes;
        self.queue = workspace.queue;
        self.track_templates = workspace.track_templates;

        for project in &mut self.queue {
            if matches!(
                project.status,
                ProjectStatus::Pending | ProjectStatus::Processing
            ) {
                project.status = ProjectStatus::Paused;
            }
        }

        self.persistence_path = Some(config_dir.to_path_buf());
        self.persist_workspace()
    }

    pub fn set_persistence_error(&mut self, error: String, config_dir: &Path) {
        self.persistence_path = Some(config_dir.to_path_buf());
        self.persistence_error = Some(error);
    }

    pub fn persist_workspace(&self) -> Result<(), String> {
        self.ensure_workspace_available()?;
        let config_dir = self
            .persistence_path
            .as_deref()
            .ok_or_else(|| "Workspace persistence is not configured".to_string())?;
        WorkspaceData {
            current_volumes: self.current_volumes.clone(),
            queue: self.queue.clone(),
            track_templates: self.track_templates.clone(),
        }
        .save(config_dir)
    }

    pub fn ensure_workspace_available(&self) -> Result<(), String> {
        if let Some(error) = &self.persistence_error {
            return Err(format!(
                "Saved work could not be restored, so it was not overwritten: {}",
                error
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{AppState, Project, ProjectStatus, Settings};
    use crate::bdmv::{Track, Volume};
    use chrono::Utc;
    use std::collections::HashMap;
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

    #[test]
    fn workspace_round_trip_restores_volumes_templates_and_pauses_incomplete_projects() {
        let config_dir = temporary_config_dir();
        let volume = Volume {
            id: "volume-1".into(),
            name: "Disc 1".into(),
            volume_number: 1,
            files: Vec::new(),
        };
        let track = Track {
            id: 2,
            track_type: "audio".into(),
            codec: "AAC".into(),
            language: "eng".into(),
            name: "Edited track name".into(),
            default: true,
            forced: false,
            resolution: None,
            channels: Some("2.0".into()),
        };
        let project = Project {
            id: "project-1".into(),
            output_name: "Show".into(),
            output_path: "/output".into(),
            volumes: vec![volume.clone()],
            track_templates: HashMap::new(),
            status: ProjectStatus::Processing,
            progress: 35.0,
            current_file: Some("00001.m2ts".into()),
            created_at: Utc::now(),
            error: None,
        };
        let state = AppState {
            current_volumes: vec![volume],
            queue: vec![project],
            track_templates: HashMap::from([("EPISODE".into(), vec![track])]),
            persistence_path: Some(config_dir.clone()),
            ..AppState::default()
        };

        state.persist_workspace().unwrap();

        let mut restored = AppState::default();
        restored.restore_workspace(&config_dir).unwrap();

        assert_eq!(restored.current_volumes[0].name, "Disc 1");
        assert_eq!(
            restored.track_templates["EPISODE"][0].name,
            "Edited track name"
        );
        assert_eq!(restored.queue[0].status, ProjectStatus::Paused);
        assert_eq!(restored.queue[0].progress, 35.0);
        assert_eq!(
            restored.queue[0].current_file.as_deref(),
            Some("00001.m2ts")
        );

        std::fs::remove_dir_all(config_dir).unwrap();
    }

    #[test]
    fn workspace_load_does_not_overwrite_invalid_saved_data() {
        let config_dir = temporary_config_dir();
        std::fs::create_dir_all(&config_dir).unwrap();
        let path = config_dir.join("workspace.json");
        std::fs::write(&path, "{ invalid json").unwrap();

        let mut state = AppState::default();
        let error = state.restore_workspace(&config_dir).unwrap_err();
        state.set_persistence_error(error, &config_dir);

        assert!(state.persist_workspace().is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ invalid json");

        std::fs::remove_dir_all(config_dir).unwrap();
    }
}
