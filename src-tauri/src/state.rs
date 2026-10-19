use crate::bdmv::{Track, Volume};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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
    const RECOVERY_FIELD_NAMES: [&'static str; 8] = [
        "mkvmerge_path",
        "ffprobe_path",
        "min_episode_duration",
        "min_extra_duration",
        "op_ed_min_duration",
        "op_ed_max_duration",
        "episode_folder_name",
        "extra_folder_name",
    ];

    pub fn preview_recovery_backup(path: &Path) -> Result<SettingsRecoveryPreview, String> {
        let contents = fs::read(path).map_err(|error| {
            format!(
                "Could not read the recovered preferences backup at {}: {}",
                path.display(),
                error
            )
        })?;
        let value: serde_json::Value = serde_json::from_slice(&contents).map_err(|error| {
            format!(
                "The recovered preferences backup is not valid JSON: {}",
                error
            )
        })?;
        let object = value.as_object().ok_or_else(|| {
            "The recovered preferences backup must contain a JSON object of preferences."
                .to_string()
        })?;

        let mut settings = Self::default();
        let mut warnings = Vec::new();
        let mut recognized_fields = 0;

        for name in object.keys() {
            if Self::RECOVERY_FIELD_NAMES.contains(&name.as_str()) {
                recognized_fields += 1;
            } else {
                warnings.push(format!(
                    "Unrecognized preference '{}' will be ignored.",
                    name
                ));
            }
        }

        restore_string_field(
            object,
            "mkvmerge_path",
            "MKVToolNix path",
            &mut settings.mkvmerge_path,
            &mut warnings,
        );
        restore_string_field(
            object,
            "ffprobe_path",
            "FFprobe path",
            &mut settings.ffprobe_path,
            &mut warnings,
        );
        restore_positive_integer_field(
            object,
            "min_episode_duration",
            "Minimum episode duration",
            &mut settings.min_episode_duration,
            &mut warnings,
        );
        restore_positive_integer_field(
            object,
            "min_extra_duration",
            "Minimum extra duration",
            &mut settings.min_extra_duration,
            &mut warnings,
        );
        restore_positive_integer_field(
            object,
            "op_ed_min_duration",
            "OP/ED minimum duration",
            &mut settings.op_ed_min_duration,
            &mut warnings,
        );
        restore_positive_integer_field(
            object,
            "op_ed_max_duration",
            "OP/ED maximum duration",
            &mut settings.op_ed_max_duration,
            &mut warnings,
        );
        restore_string_field(
            object,
            "episode_folder_name",
            "Episode folder name",
            &mut settings.episode_folder_name,
            &mut warnings,
        );
        restore_string_field(
            object,
            "extra_folder_name",
            "Extra folder name",
            &mut settings.extra_folder_name,
            &mut warnings,
        );

        if recognized_fields == 0 {
            warnings.push(
                "No recognized preferences were found; restoring will use default values."
                    .to_string(),
            );
        }

        if settings.op_ed_min_duration > settings.op_ed_max_duration {
            settings.op_ed_min_duration = Self::default().op_ed_min_duration;
            settings.op_ed_max_duration = Self::default().op_ed_max_duration;
            warnings.push(
                "The OP/ED minimum duration is greater than its maximum; the default OP/ED range will be restored."
                    .to_string(),
            );
        }

        Ok(SettingsRecoveryPreview {
            settings,
            warnings,
            source_hash: format!("{:x}", Sha256::digest(&contents)),
        })
    }

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
        write_settings_atomically(&path, &contents)
    }
}

#[derive(Debug, Serialize)]
pub struct SettingsRecoveryPreview {
    pub settings: Settings,
    pub warnings: Vec<String>,
    pub source_hash: String,
}

fn restore_string_field(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &str,
    label: &str,
    target: &mut String,
    warnings: &mut Vec<String>,
) {
    let Some(value) = object.get(field) else {
        return;
    };
    match value.as_str().filter(|value| !value.trim().is_empty()) {
        Some(value) => *target = value.to_string(),
        None => warnings.push(format!(
            "{} was invalid or empty; the default value will be used.",
            label
        )),
    }
}

fn restore_positive_integer_field(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &str,
    label: &str,
    target: &mut i64,
    warnings: &mut Vec<String>,
) {
    let Some(value) = object.get(field) else {
        return;
    };
    match value.as_i64().filter(|value| *value > 0) {
        Some(value) => *target = value,
        None => warnings.push(format!(
            "{} was invalid; a positive whole number is required, so the default value will be used.",
            label
        )),
    }
}

fn write_settings_atomically(path: &Path, contents: &[u8]) -> Result<(), String> {
    write_settings_atomically_with(path, contents, replace_file)
}

fn write_settings_atomically_with(
    path: &Path,
    contents: &[u8],
    replace: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> Result<(), String> {
    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("settings"));
    let (temporary_path, mut temporary_file) = loop {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary_path = parent.join(format!(
            ".{}.tmp-{}-{}",
            file_name.to_string_lossy(),
            std::process::id(),
            counter
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => break (temporary_path, file),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "Failed to create temporary settings file beside {}: {}",
                    path.display(),
                    error
                ));
            }
        }
    };

    if let Err(error) = temporary_file
        .write_all(contents)
        .and_then(|()| temporary_file.sync_all())
    {
        drop(temporary_file);
        let _ = fs::remove_file(&temporary_path);
        return Err(format!(
            "Failed to write temporary settings file for {}: {}",
            path.display(),
            error
        ));
    }
    drop(temporary_file);

    if let Err(error) = replace(&temporary_path, path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(format!(
            "Failed to replace settings at {}: {}",
            path.display(),
            error
        ));
    }

    Ok(())
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    #[link(name = "Kernel32")]
    extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let succeeded = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };

    if succeeded == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
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
    pub completed_files: Vec<CompletedFile>,
    pub created_at: DateTime<Utc>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CompletedFile {
    pub path: String,
    pub fingerprint: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CompletedFileRecord {
    Legacy(String),
    Current {
        path: String,
        #[serde(default)]
        fingerprint: Option<String>,
    },
}

impl<'de> Deserialize<'de> for CompletedFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(match CompletedFileRecord::deserialize(deserializer)? {
            CompletedFileRecord::Legacy(path) => Self {
                path,
                fingerprint: None,
            },
            CompletedFileRecord::Current { path, fingerprint } => Self { path, fingerprint },
        })
    }
}

pub fn source_file_fingerprint(file_path: &str) -> Result<String, String> {
    let mut file = fs::File::open(file_path)
        .map_err(|error| format!("Failed to read source file {}: {}", file_path, error))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 64 * 1024];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|error| format!("Failed to read source file {}: {}", file_path, error))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

impl Project {
    pub fn is_file_completed(&self, file_path: &str) -> bool {
        self.completed_files
            .iter()
            .any(|completed| completed.path == file_path)
    }

    pub fn retain_completed_files_matching(&mut self, fingerprints: &HashMap<String, String>) {
        self.completed_files.retain(|completed| {
            completed.fingerprint.as_ref().is_some_and(|fingerprint| {
                fingerprints
                    .get(&completed.path)
                    .is_some_and(|current| current == fingerprint)
            })
        });
    }

    pub fn mark_file_completed(&mut self, file_path: &str, fingerprint: String) {
        if let Some(completed) = self
            .completed_files
            .iter_mut()
            .find(|completed| completed.path == file_path)
        {
            completed.fingerprint = Some(fingerprint);
        } else {
            self.completed_files.push(CompletedFile {
                path: file_path.to_string(),
                fingerprint: Some(fingerprint),
            });
        }
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

    pub fn prepare_retry(&mut self) {
        self.status = ProjectStatus::Pending;
        self.progress = self.progress_for_completed_files();
        self.current_file = None;
        self.error = None;
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
        settings_directory_for_config_root, source_file_fingerprint,
        write_settings_atomically_with, AppState, CompletedFile, Project, ProjectStatus, Settings,
        WorkspaceData,
    };
    use crate::bdmv::{ProbeFailure, Track, Volume};
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
    fn settings_from_before_newer_fields_were_added_keep_saved_values_and_default_missing_fields() {
        let config_dir = temporary_config_dir();
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("settings.json"),
            include_str!("../tests/fixtures/settings-before-newer-fields.json"),
        )
        .unwrap();

        let outcome = Settings::load_with_recovery(&config_dir);
        let defaults = Settings::default();

        assert!(outcome.recovery_warning.is_none());
        assert!(outcome.can_save);
        assert_eq!(outcome.settings.mkvmerge_path, "/opt/tools/mkvmerge");
        assert_eq!(outcome.settings.ffprobe_path, "/opt/tools/ffprobe");
        assert_eq!(outcome.settings.min_episode_duration, 1500);
        assert_eq!(outcome.settings.min_extra_duration, 180);
        assert_eq!(
            outcome.settings.op_ed_min_duration,
            defaults.op_ed_min_duration
        );
        assert_eq!(
            outcome.settings.op_ed_max_duration,
            defaults.op_ed_max_duration
        );
        assert_eq!(
            outcome.settings.episode_folder_name,
            defaults.episode_folder_name
        );
        assert_eq!(
            outcome.settings.extra_folder_name,
            defaults.extra_folder_name
        );

        std::fs::remove_dir_all(config_dir).unwrap();
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
    fn recovery_backup_preview_keeps_valid_values_and_explains_invalid_entries() {
        let config_dir = temporary_config_dir();
        std::fs::create_dir_all(&config_dir).unwrap();
        let backup_path = config_dir.join("settings.json.corrupt");
        let backup_contents = br#"{
            "mkvmerge_path": "",
            "ffprobe_path": 27,
            "min_episode_duration": -5,
            "min_extra_duration": 240,
            "op_ed_min_duration": 130,
            "op_ed_max_duration": 100,
            "episode_folder_name": "Episodes",
            "unexpected_setting": true
        }"#;
        std::fs::write(&backup_path, backup_contents).unwrap();

        let preview = Settings::preview_recovery_backup(&backup_path).unwrap();
        let defaults = Settings::default();

        assert_eq!(preview.settings.mkvmerge_path, defaults.mkvmerge_path);
        assert_eq!(preview.settings.ffprobe_path, defaults.ffprobe_path);
        assert_eq!(
            preview.settings.min_episode_duration,
            defaults.min_episode_duration
        );
        assert_eq!(preview.settings.min_extra_duration, 240);
        assert_eq!(
            preview.settings.op_ed_min_duration,
            defaults.op_ed_min_duration
        );
        assert_eq!(
            preview.settings.op_ed_max_duration,
            defaults.op_ed_max_duration
        );
        assert_eq!(preview.settings.episode_folder_name, "Episodes");
        assert!(preview
            .warnings
            .iter()
            .any(|warning| warning.contains("MKVToolNix path")));
        assert!(preview
            .warnings
            .iter()
            .any(|warning| warning.contains("FFprobe path")));
        assert!(preview
            .warnings
            .iter()
            .any(|warning| warning.contains("Minimum episode duration")));
        assert!(preview
            .warnings
            .iter()
            .any(|warning| warning.contains("OP/ED minimum duration is greater")));
        assert!(preview
            .warnings
            .iter()
            .any(|warning| warning.contains("unexpected_setting")));
        assert_eq!(std::fs::read(&backup_path).unwrap(), backup_contents);

        std::fs::remove_dir_all(config_dir).unwrap();
    }

    #[test]
    fn restored_settings_can_be_saved_without_changing_recovery_backup() {
        let config_dir = temporary_config_dir();
        std::fs::create_dir_all(&config_dir).unwrap();
        let backup_path = config_dir.join("settings.json.corrupt");
        let backup_contents = br#"{"mkvmerge_path":"/tools/mkvmerge","min_extra_duration":240}"#;
        std::fs::write(&backup_path, backup_contents).unwrap();

        let preview = Settings::preview_recovery_backup(&backup_path).unwrap();
        preview.settings.save_to_config_dir(&config_dir).unwrap();

        let saved: Settings =
            serde_json::from_slice(&std::fs::read(config_dir.join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(saved.mkvmerge_path, "/tools/mkvmerge");
        assert_eq!(saved.min_extra_duration, 240);
        assert_eq!(std::fs::read(&backup_path).unwrap(), backup_contents);

        std::fs::remove_dir_all(config_dir).unwrap();
    }

    #[test]
    fn recovery_preview_hash_changes_when_the_backup_is_edited() {
        let config_dir = temporary_config_dir();
        std::fs::create_dir_all(&config_dir).unwrap();
        let backup_path = config_dir.join("settings.json.corrupt");
        std::fs::write(&backup_path, br#"{"mkvmerge_path":"mkvmerge"}"#).unwrap();

        let reviewed = Settings::preview_recovery_backup(&backup_path).unwrap();
        std::fs::write(&backup_path, br#"{"mkvmerge_path":"different"}"#).unwrap();
        let edited = Settings::preview_recovery_backup(&backup_path).unwrap();

        assert_ne!(reviewed.source_hash, edited.source_hash);
        std::fs::remove_dir_all(config_dir).unwrap();
    }

    #[test]
    fn recovery_preview_rejects_non_object_and_malformed_backup_files() {
        let config_dir = temporary_config_dir();
        std::fs::create_dir_all(&config_dir).unwrap();
        let backup_path = config_dir.join("settings.json.corrupt");

        std::fs::write(&backup_path, "[]").unwrap();
        assert!(Settings::preview_recovery_backup(&backup_path)
            .unwrap_err()
            .contains("must contain a JSON object"));

        std::fs::write(&backup_path, "{ broken").unwrap();
        assert!(Settings::preview_recovery_backup(&backup_path)
            .unwrap_err()
            .contains("not valid JSON"));

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
    fn saving_settings_replaces_existing_file_and_cleans_up_temporary_file() {
        let config_dir = temporary_config_dir();
        std::fs::create_dir_all(&config_dir).unwrap();
        let settings_path = config_dir.join("settings.json");
        std::fs::write(&settings_path, "previous settings").unwrap();

        let mut settings = Settings::default();
        settings.mkvmerge_path = "/custom/bin/mkvmerge".into();
        settings.save_to_config_dir(&config_dir).unwrap();

        let saved: Settings =
            serde_json::from_slice(&std::fs::read(&settings_path).unwrap()).unwrap();
        assert_eq!(saved.mkvmerge_path, "/custom/bin/mkvmerge");
        assert_eq!(
            std::fs::read_dir(&config_dir)
                .unwrap()
                .filter_map(Result::ok)
                .count(),
            1,
            "temporary settings file should be removed after replacement"
        );

        std::fs::remove_dir_all(config_dir).unwrap();
    }

    #[test]
    fn failed_settings_replacement_preserves_existing_file_and_cleans_up_temporary_file() {
        let config_dir = temporary_config_dir();
        std::fs::create_dir_all(&config_dir).unwrap();
        let settings_path = config_dir.join("settings.json");
        let previous_settings = br#"{"mkvmerge_path":"existing"}"#;
        std::fs::write(&settings_path, previous_settings).unwrap();

        let error = write_settings_atomically_with(
            &settings_path,
            br#"{"mkvmerge_path":"new"}"#,
            |_, _| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "simulated replacement failure",
                ))
            },
        )
        .unwrap_err();

        assert!(error.contains("simulated replacement failure"));
        assert_eq!(std::fs::read(&settings_path).unwrap(), previous_settings);
        assert_eq!(
            std::fs::read_dir(&config_dir)
                .unwrap()
                .filter_map(Result::ok)
                .count(),
            1,
            "temporary settings file should be removed after failed replacement"
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
            probe_failures: vec![ProbeFailure {
                path: "/disc/BDMV/STREAM/bad.m2ts".into(),
                diagnostic: "Invalid data found when processing input".into(),
            }],
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
            completed_files: vec![CompletedFile {
                path: "/disc/BDMV/STREAM/00000.m2ts".into(),
                fingerprint: Some("sha256:fixture".into()),
            }],
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
        assert_eq!(restored.current_volumes[0].probe_failures.len(), 1);
        assert_eq!(
            restored.current_volumes[0].probe_failures[0].diagnostic,
            "Invalid data found when processing input"
        );
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
            vec![CompletedFile {
                path: "/disc/BDMV/STREAM/00000.m2ts".into(),
                fingerprint: Some("sha256:fixture".into()),
            }]
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
                probe_failures: Vec::new(),
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

        project.mark_file_completed("/disc/BDMV/STREAM/00001.m2ts", "fingerprint-1".into());
        assert_eq!(project.completed_file_count(), 1);
        assert_eq!(project.first_unfinished_file_index(), Some(1));
        assert!((project.progress_for_completed_files() - 100.0 / 3.0).abs() < 0.001);
        assert!(project.is_file_completed("/disc/BDMV/STREAM/00001.m2ts"));
        assert!(!project.is_file_completed("/disc/BDMV/STREAM/00002.m2ts"));

        project.status = ProjectStatus::Error;
        project.progress = 100.0;
        project.current_file = Some("00002.m2ts".into());
        project.error = Some("mux failed".into());
        project.prepare_retry();

        assert_eq!(project.status, ProjectStatus::Pending);
        assert_eq!(project.current_file, None);
        assert_eq!(project.error, None);
        assert_eq!(
            project.completed_files,
            vec![CompletedFile {
                path: "/disc/BDMV/STREAM/00001.m2ts".into(),
                fingerprint: Some("fingerprint-1".into()),
            }]
        );
        assert!((project.progress - 100.0 / 3.0).abs() < 0.001);

        project.mark_file_completed("/disc/BDMV/STREAM/00002.m2ts", "fingerprint-2".into());
        project.mark_file_completed("/disc/BDMV/STREAM/00003.m2ts", "fingerprint-3".into());
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
    fn replaced_source_at_same_path_is_unfinished_on_resume() {
        let source_path = temporary_config_dir().join("source.m2ts");
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        std::fs::write(&source_path, b"original file bytes").unwrap();
        let source_path = source_path.to_string_lossy().into_owned();
        let original_fingerprint = source_file_fingerprint(&source_path).unwrap();
        let mut file = test_media_file("source.m2ts");
        file.path = source_path.clone();
        let mut project = Project {
            id: "replacement-resume".into(),
            output_name: "Show".into(),
            output_path: "/output".into(),
            volumes: vec![Volume {
                id: "volume-1".into(),
                name: "Disc 1".into(),
                volume_number: 1,
                files: vec![file],
                probe_failures: Vec::new(),
            }],
            track_templates: HashMap::new(),
            status: ProjectStatus::Paused,
            progress: 100.0,
            current_file: None,
            completed_files: vec![CompletedFile {
                path: source_path.clone(),
                fingerprint: Some(original_fingerprint.clone()),
            }],
            created_at: Utc::now(),
            error: None,
        };

        // Replace the contents without changing the source path.
        std::fs::write(&source_path, b"replaced file bytes").unwrap();
        let fingerprints = HashMap::from([(
            source_path.clone(),
            source_file_fingerprint(&source_path).unwrap(),
        )]);
        assert_ne!(fingerprints[&source_path], original_fingerprint);
        project.retain_completed_files_matching(&fingerprints);

        assert_eq!(project.completed_file_count(), 0);
        assert_eq!(project.first_unfinished_file_index(), Some(0));
        std::fs::remove_dir_all(std::path::Path::new(&source_path).parent().unwrap()).unwrap();
    }

    #[test]
    fn unchanged_source_remains_completed_on_resume() {
        let source_path = temporary_config_dir().join("source.m2ts");
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        std::fs::write(&source_path, b"unchanged file bytes").unwrap();
        let source_path = source_path.to_string_lossy().into_owned();
        let fingerprint = source_file_fingerprint(&source_path).unwrap();
        let mut file = test_media_file("source.m2ts");
        file.path = source_path.clone();
        let mut project = Project {
            id: "unchanged-resume".into(),
            output_name: "Show".into(),
            output_path: "/output".into(),
            volumes: vec![Volume {
                id: "volume-1".into(),
                name: "Disc 1".into(),
                volume_number: 1,
                files: vec![file],
                probe_failures: Vec::new(),
            }],
            track_templates: HashMap::new(),
            status: ProjectStatus::Paused,
            progress: 100.0,
            current_file: None,
            completed_files: vec![CompletedFile {
                path: source_path.clone(),
                fingerprint: Some(fingerprint.clone()),
            }],
            created_at: Utc::now(),
            error: None,
        };

        project.retain_completed_files_matching(&HashMap::from([(
            source_path.clone(),
            source_file_fingerprint(&source_path).unwrap(),
        )]));

        assert_eq!(project.completed_file_count(), 1);
        assert_eq!(project.first_unfinished_file_index(), None);
        assert!(project.is_file_completed(&source_path));
        std::fs::remove_dir_all(std::path::Path::new(&source_path).parent().unwrap()).unwrap();
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
    fn old_path_only_completion_markers_load_without_trusting_missing_fingerprints() {
        let mut project: Project = serde_json::from_str(
            r#"{
                "id": "legacy-project",
                "output_name": "Show",
                "output_path": "/output",
                "volumes": [],
                "track_templates": {},
                "status": "paused",
                "progress": 0.0,
                "current_file": null,
                "completed_files": ["/disc/BDMV/STREAM/00001.m2ts"],
                "created_at": "2024-01-01T00:00:00Z",
                "error": null
            }"#,
        )
        .unwrap();

        assert_eq!(project.completed_files[0].fingerprint, None);
        project.retain_completed_files_matching(&HashMap::new());
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
