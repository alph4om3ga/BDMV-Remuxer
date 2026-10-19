use crate::bdmv::{Track, Volume};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{ErrorKind, Write};
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

    pub fn load_with_recovery(config_dir: &Path) -> SettingsLoadOutcome {
        let path = config_dir.join(Self::FILE_NAME);
        match fs::read(&path) {
            Ok(contents) => match serde_json::from_slice(&contents) {
                Ok(settings) => SettingsLoadOutcome {
                    settings,
                    recovery_warning: None,
                    recovery_backup_path: None,
                    can_save: true,
                },
                Err(parse_error) => match preserve_damaged_settings(config_dir, &contents) {
                    Ok(backup_path) => SettingsLoadOutcome {
                        settings: Self::default(),
                        recovery_warning: Some(format!(
                            "Saved preferences could not be loaded: {}. Defaults are active. \
                             The damaged file was preserved at {}. Open Settings to restore \
                             preferences manually or save new settings to reset them.",
                            parse_error,
                            backup_path.display()
                        )),
                        recovery_backup_path: Some(backup_path),
                        can_save: true,
                    },
                    Err(backup_error) => SettingsLoadOutcome {
                        settings: Self::default(),
                        recovery_warning: Some(format!(
                            "Saved preferences could not be loaded: {}. A recovery copy could \
                             not be created ({}), so saving settings is disabled to protect \
                             the damaged file at {}.",
                            parse_error,
                            backup_error,
                            path.display()
                        )),
                        recovery_backup_path: None,
                        can_save: false,
                    },
                },
            },
            Err(error) if error.kind() == ErrorKind::NotFound => SettingsLoadOutcome {
                settings: Self::default(),
                recovery_warning: None,
                recovery_backup_path: None,
                can_save: true,
            },
            Err(error) => SettingsLoadOutcome {
                settings: Self::default(),
                recovery_warning: Some(format!(
                    "Saved preferences could not be read at {}: {}. Defaults are active; \
                     saving is disabled to avoid replacing preferences that could not be read.",
                    path.display(),
                    error
                )),
                recovery_backup_path: None,
                can_save: false,
            },
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

pub fn settings_directory_for_config_root(config_root: &Path) -> PathBuf {
    config_root.join("bdmv-remuxer")
}

pub struct SettingsLoadOutcome {
    pub settings: Settings,
    pub recovery_warning: Option<String>,
    pub recovery_backup_path: Option<PathBuf>,
    pub can_save: bool,
}

fn preserve_damaged_settings(config_dir: &Path, contents: &[u8]) -> Result<PathBuf, String> {
    let settings_path = config_dir.join(Settings::FILE_NAME);
    let mut backup_path = config_dir.join(format!("{}.corrupt", Settings::FILE_NAME));
    let mut suffix = 1;
    loop {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup_path)
        {
            Ok(mut backup_file) => {
                if let Err(error) = backup_file.write_all(contents) {
                    let _ = fs::remove_file(&backup_path);
                    return Err(format!(
                        "Failed to preserve damaged settings from {} to {}: {}",
                        settings_path.display(),
                        backup_path.display(),
                        error
                    ));
                }
                return Ok(backup_path);
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                backup_path =
                    config_dir.join(format!("{}.corrupt.{}", Settings::FILE_NAME, suffix));
                suffix += 1;
            }
            Err(error) => {
                return Err(format!(
                    "Failed to preserve damaged settings from {} to {}: {}",
                    settings_path.display(),
                    backup_path.display(),
                    error
                ));
            }
        }
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
    #[serde(default)]
    pub completed_files: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub error: Option<String>,
}

impl Project {
    pub fn is_file_completed(&self, file_path: &str) -> bool {
        self.completed_files
            .iter()
            .any(|completed| completed == file_path)
    }

    pub fn first_unfinished_file_index(&self) -> Option<usize> {
        self.volumes
            .iter()
            .flat_map(|volume| &volume.files)
            .enumerate()
            .find(|(_, file)| !self.is_file_completed(&file.path))
            .map(|(index, _)| index)
    }

    pub fn completed_file_count(&self) -> usize {
        self.volumes
            .iter()
            .flat_map(|volume| &volume.files)
            .filter(|file| self.is_file_completed(&file.path))
            .count()
    }

    pub fn total_file_count(&self) -> usize {
        self.volumes.iter().map(|volume| volume.files.len()).sum()
    }

    pub fn progress_for_completed_files(&self) -> f64 {
        let total_files = self.total_file_count();
        if total_files == 0 {
            0.0
        } else {
            (self.completed_file_count() as f64 / total_files as f64) * 100.0
        }
    }
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
    settings_recovery_warning: Option<String>,
    settings_recovery_backup_path: Option<PathBuf>,
    settings_save_blocked: bool,
}

impl AppState {
    pub fn set_settings_recovery(
        &mut self,
        warning: Option<String>,
        backup_path: Option<PathBuf>,
        can_save: bool,
    ) {
        self.settings_recovery_warning = warning;
        self.settings_recovery_backup_path = backup_path;
        self.settings_save_blocked = !can_save;
    }

    pub fn settings_recovery_warning(&self) -> Option<&str> {
        self.settings_recovery_warning.as_deref()
    }

    pub fn settings_recovery_backup_path(&self) -> Option<&Path> {
        self.settings_recovery_backup_path.as_deref()
    }

    pub fn ensure_settings_savable(&self) -> Result<(), String> {
        if self.settings_save_blocked {
            return Err(self.settings_recovery_warning.clone().unwrap_or_else(|| {
                "Settings cannot be saved until the damaged preferences file is preserved."
                    .to_string()
            }));
        }
        Ok(())
    }

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
            if project.status != ProjectStatus::Complete {
                project.progress = project.progress_for_completed_files();
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

    pub fn pause_incomplete_projects(&mut self) -> Result<(), String> {
        for project in &mut self.queue {
            if matches!(
                project.status,
                ProjectStatus::Pending | ProjectStatus::Processing
            ) {
                project.status = ProjectStatus::Paused;
            }
        }

        self.persist_workspace()
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
    use super::{
        settings_directory_for_config_root, AppState, Project, ProjectStatus, Settings,
        WorkspaceData,
    };
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

    fn test_media_file(filename: &str) -> crate::bdmv::MediaFile {
        crate::bdmv::MediaFile {
            id: filename.into(),
            filename: filename.into(),
            path: format!("/disc/BDMV/STREAM/{filename}"),
            duration: 0.0,
            size: 0,
            category: "EPISODE".into(),
            tracks: Vec::new(),
            episode_number: None,
            op_number: None,
            ed_number: None,
            special_number: None,
            chapter_file: None,
        }
    }

    #[test]
    fn settings_directory_uses_the_expected_app_folder_for_each_platform_root() {
        let platform_roots = [
            (
                "Windows roaming config",
                PathBuf::from("C:/Users/Alice/AppData/Roaming"),
                PathBuf::from("C:/Users/Alice/AppData/Roaming/bdmv-remuxer"),
            ),
            (
                "macOS Application Support",
                PathBuf::from("/Users/alice/Library/Application Support"),
                PathBuf::from("/Users/alice/Library/Application Support/bdmv-remuxer"),
            ),
            (
                "Linux XDG config",
                PathBuf::from("/home/alice/.config"),
                PathBuf::from("/home/alice/.config/bdmv-remuxer"),
            ),
        ];

        for (platform, config_root, expected) in platform_roots {
            assert_eq!(
                settings_directory_for_config_root(&config_root),
                expected,
                "unexpected settings directory for {platform}"
            );
        }
    }

    #[test]
    fn missing_settings_file_uses_defaults() {
        let config_dir = temporary_config_dir();
        let settings = Settings::load_with_recovery(&config_dir);

        assert_eq!(
            settings.settings.mkvmerge_path,
            Settings::default().mkvmerge_path
        );
        assert_eq!(
            settings.settings.min_episode_duration,
            Settings::default().min_episode_duration
        );
        assert!(settings.recovery_warning.is_none());
        assert!(settings.can_save);
    }

    #[test]
    fn malformed_settings_are_preserved_and_defaults_can_be_saved_as_a_reset() {
        let config_dir = temporary_config_dir();
        std::fs::create_dir_all(&config_dir).unwrap();
        let settings_path = config_dir.join("settings.json");
        let damaged_contents = "{ invalid json";
        std::fs::write(&settings_path, damaged_contents).unwrap();

        let outcome = Settings::load_with_recovery(&config_dir);
        let warning = outcome.recovery_warning.as_deref().unwrap();
        assert!(warning.contains("Saved preferences could not be loaded"));
        assert!(warning.contains("settings.json.corrupt"));
        assert!(outcome.can_save);
        assert_eq!(
            outcome.settings.mkvmerge_path,
            Settings::default().mkvmerge_path
        );
        assert_eq!(
            std::fs::read_to_string(&settings_path).unwrap(),
            damaged_contents
        );

        let backup_path = config_dir.join("settings.json.corrupt");
        assert_eq!(
            outcome.recovery_backup_path.as_deref(),
            Some(backup_path.as_path())
        );
        assert_eq!(
            std::fs::read_to_string(&backup_path).unwrap(),
            damaged_contents
        );

        outcome.settings.save_to_config_dir(&config_dir).unwrap();
        assert_ne!(
            std::fs::read_to_string(&settings_path).unwrap(),
            damaged_contents
        );
        assert_eq!(
            std::fs::read_to_string(&backup_path).unwrap(),
            damaged_contents
        );

        std::fs::remove_dir_all(config_dir).unwrap();
    }

    #[test]
    fn malformed_settings_use_a_new_backup_name_without_replacing_an_existing_backup() {
        let config_dir = temporary_config_dir();
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("settings.json"), "{ broken again").unwrap();
        std::fs::write(config_dir.join("settings.json.corrupt"), "earlier recovery").unwrap();

        let outcome = Settings::load_with_recovery(&config_dir);

        assert!(outcome.can_save);
        assert_eq!(
            std::fs::read_to_string(config_dir.join("settings.json.corrupt")).unwrap(),
            "earlier recovery"
        );
        assert_eq!(
            std::fs::read_to_string(config_dir.join("settings.json.corrupt.1")).unwrap(),
            "{ broken again"
        );

        std::fs::remove_dir_all(config_dir).unwrap();
    }

    #[test]
    fn settings_saving_is_blocked_when_recovery_copy_could_not_be_made() {
        let mut state = AppState::default();
        state.set_settings_recovery(
            Some("Could not preserve damaged preferences.".into()),
            None,
            false,
        );

        let error = state.ensure_settings_savable().unwrap_err();
        assert!(error.contains("Could not preserve damaged preferences"));
    }

    #[test]
    fn settings_survive_save_and_reload_after_restart() {
        let config_dir = temporary_config_dir();
        let mut settings = Settings::default();
        settings.mkvmerge_path = "/custom/bin/mkvmerge".into();
        settings.ffprobe_path = "/custom/bin/ffprobe".into();
        settings.min_episode_duration = 1800;
        settings.min_extra_duration = 240;
        settings.op_ed_min_duration = 75;
        settings.op_ed_max_duration = 125;
        settings.episode_folder_name = "Episodes".into();
        settings.extra_folder_name = "Specials".into();

        settings.save_to_config_dir(&config_dir).unwrap();
        let first_load = Settings::load_with_recovery(&config_dir);
        assert!(first_load.recovery_warning.is_none());
        assert!(first_load.can_save);
        assert_eq!(
            serde_json::to_value(first_load.settings).unwrap(),
            serde_json::to_value(&settings).unwrap()
        );

        // A fresh load represents the next app launch; it must recover the same preferences
        // from disk rather than relying on the first process's in-memory state.
        let restarted_load = Settings::load_with_recovery(&config_dir);
        assert!(restarted_load.recovery_warning.is_none());
        assert!(restarted_load.can_save);
        assert_eq!(
            serde_json::to_value(restarted_load.settings).unwrap(),
            serde_json::to_value(settings).unwrap()
        );

        std::fs::remove_dir_all(config_dir).unwrap();
    }

    #[test]
    fn workspace_round_trip_restores_volumes_templates_and_pauses_incomplete_projects() {
        let config_dir = temporary_config_dir();
        let volume = Volume {
            id: "volume-1".into(),
            name: "Disc 1".into(),
            volume_number: 1,
            files: vec![
                test_media_file("00000.m2ts"),
                test_media_file("00001.m2ts"),
                test_media_file("00002.m2ts"),
            ],
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
            progress: 89.0,
            current_file: Some("00001.m2ts".into()),
            completed_files: vec!["/disc/BDMV/STREAM/00000.m2ts".into()],
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
        assert!((restored.queue[0].progress - 100.0 / 3.0).abs() < 0.001);
        assert_eq!(
            restored.queue[0].current_file.as_deref(),
            Some("00001.m2ts")
        );
        assert_eq!(
            restored.queue[0].completed_files,
            vec!["/disc/BDMV/STREAM/00000.m2ts"]
        );
        assert_eq!(restored.queue[0].first_unfinished_file_index(), Some(1));
        assert_eq!(restored.queue[0].completed_file_count(), 1);

        std::fs::remove_dir_all(config_dir).unwrap();
    }

    #[test]
    fn pausing_incomplete_projects_persists_before_close_and_keeps_completed_projects_complete() {
        let config_dir = temporary_config_dir();
        let make_project = |id: &str, status| Project {
            id: id.into(),
            output_name: id.into(),
            output_path: "/output".into(),
            volumes: Vec::new(),
            track_templates: HashMap::new(),
            status,
            progress: 0.0,
            current_file: None,
            completed_files: Vec::new(),
            created_at: Utc::now(),
            error: None,
        };
        let mut state = AppState {
            queue: vec![
                make_project("pending", ProjectStatus::Pending),
                make_project("processing", ProjectStatus::Processing),
                make_project("complete", ProjectStatus::Complete),
            ],
            persistence_path: Some(config_dir.clone()),
            ..AppState::default()
        };

        state.pause_incomplete_projects().unwrap();

        let saved = WorkspaceData::load(&config_dir).unwrap();
        assert_eq!(saved.queue[0].status, ProjectStatus::Paused);
        assert_eq!(saved.queue[1].status, ProjectStatus::Paused);
        assert_eq!(saved.queue[2].status, ProjectStatus::Complete);

        std::fs::remove_dir_all(config_dir).unwrap();
    }

    #[test]
    fn resume_progress_counts_completed_paths_at_the_beginning_middle_and_end() {
        use crate::bdmv::MediaFile;

        let make_file = |filename: &str| MediaFile {
            id: filename.into(),
            filename: filename.into(),
            path: format!("/disc/BDMV/STREAM/{filename}"),
            duration: 0.0,
            size: 0,
            category: "EPISODE".into(),
            tracks: Vec::new(),
            episode_number: None,
            op_number: None,
            ed_number: None,
            special_number: None,
            chapter_file: None,
        };
        let mut project = Project {
            id: "resume-project".into(),
            output_name: "Show".into(),
            output_path: "/output".into(),
            volumes: vec![Volume {
                id: "volume-1".into(),
                name: "Disc 1".into(),
                volume_number: 1,
                files: vec![
                    make_file("00001.m2ts"),
                    make_file("00002.m2ts"),
                    make_file("00003.m2ts"),
                ],
            }],
            track_templates: HashMap::new(),
            status: ProjectStatus::Paused,
            progress: 0.0,
            current_file: None,
            completed_files: Vec::new(),
            created_at: Utc::now(),
            error: None,
        };

        assert_eq!(project.completed_file_count(), 0);
        assert_eq!(project.progress_for_completed_files(), 0.0);
        assert_eq!(project.first_unfinished_file_index(), Some(0));
        assert!(!project.is_file_completed("/disc/BDMV/STREAM/00001.m2ts"));

        project
            .completed_files
            .push("/disc/BDMV/STREAM/00001.m2ts".into());
        assert_eq!(project.completed_file_count(), 1);
        assert_eq!(project.first_unfinished_file_index(), Some(1));
        assert!((project.progress_for_completed_files() - 100.0 / 3.0).abs() < 0.001);
        assert!(project.is_file_completed("/disc/BDMV/STREAM/00001.m2ts"));
        assert!(!project.is_file_completed("/disc/BDMV/STREAM/00002.m2ts"));

        project.completed_files.extend([
            "/disc/BDMV/STREAM/00002.m2ts".into(),
            "/disc/BDMV/STREAM/00003.m2ts".into(),
        ]);
        assert_eq!(project.completed_file_count(), 3);
        assert_eq!(project.first_unfinished_file_index(), None);
        assert_eq!(project.progress_for_completed_files(), 100.0);
        assert!(project
            .volumes
            .iter()
            .flat_map(|volume| &volume.files)
            .all(|file| project.is_file_completed(&file.path)));
    }

    #[test]
    fn older_saved_projects_without_completed_files_load_with_an_empty_resume_marker() {
        let project: Project = serde_json::from_str(
            r#"{
                "id": "legacy-project",
                "output_name": "Show",
                "output_path": "/output",
                "volumes": [],
                "track_templates": {},
                "status": "paused",
                "progress": 0.0,
                "current_file": null,
                "created_at": "2024-01-01T00:00:00Z",
                "error": null
            }"#,
        )
        .unwrap();

        assert!(project.completed_files.is_empty());
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
