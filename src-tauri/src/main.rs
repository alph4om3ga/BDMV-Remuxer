// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bdmv;
mod muxer;
mod state;

use crate::bdmv::{BdmvParser, MediaFile, Track, Volume};
use crate::muxer::{MuxJob, MuxProgress, Muxer};
use crate::state::{AppState, Project, ProjectStatus, Settings};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Manager, State, Window};
use uuid::Uuid;

// Type alias for our state
type AppStateType = Arc<Mutex<AppState>>;

// ============================================================================
// Commands - BDMV Loading
// ============================================================================

#[tauri::command]
async fn load_bdmv(path: String, state: State<'_, AppStateType>) -> Result<Volume, String> {
    let parser = BdmvParser::new();
    let volume = parser
        .parse_volume(&path)
        .await
        .map_err(|e| e.to_string())?;

    // Store in state
    {
        let mut app_state = state.lock();
        app_state.current_volumes.push(volume.clone());
        app_state.persist_workspace()?;
    }

    Ok(volume)
}

#[tauri::command]
async fn load_multiple_bdmvs(
    paths: Vec<String>,
    state: State<'_, AppStateType>,
) -> Result<Vec<Volume>, String> {
    let parser = BdmvParser::new();
    let mut volumes = Vec::new();

    for (index, path) in paths.iter().enumerate() {
        let mut volume = parser.parse_volume(path).await.map_err(|e| e.to_string())?;
        volume.volume_number = index + 1;
        volumes.push(volume);
    }

    // Auto-assign episode numbers across volumes
    let mut episode_counter = 1;
    for volume in &mut volumes {
        for file in &mut volume.files {
            if file.category == "EPISODE" {
                file.episode_number = Some(episode_counter);
                episode_counter += 1;
            }
        }
    }

    // Store in state
    {
        let mut app_state = state.lock();
        app_state.current_volumes = volumes.clone();
        app_state.persist_workspace()?;
    }

    Ok(volumes)
}

#[tauri::command]
async fn scan_for_volumes(base_path: String) -> Result<Vec<String>, String> {
    let parser = BdmvParser::new();
    parser
        .find_bdmv_volumes(&base_path)
        .await
        .map_err(|e| e.to_string())
}

// ============================================================================
// Commands - File Classification
// ============================================================================

#[tauri::command]
async fn update_file_category(
    volume_id: String,
    file_id: String,
    category: String,
    state: State<'_, AppStateType>,
) -> Result<(), String> {
    let mut app_state = state.lock();

    for volume in &mut app_state.current_volumes {
        if volume.id == volume_id {
            for file in &mut volume.files {
                if file.id == file_id {
                    file.category = category;
                    app_state.persist_workspace()?;
                    return Ok(());
                }
            }
        }
    }

    Err("File not found".into())
}

#[tauri::command]
async fn update_file_number(
    volume_id: String,
    file_id: String,
    number: i32,
    state: State<'_, AppStateType>,
) -> Result<(), String> {
    let mut app_state = state.lock();

    for volume in &mut app_state.current_volumes {
        if volume.id == volume_id {
            for file in &mut volume.files {
                if file.id == file_id {
                    match file.category.as_str() {
                        "EPISODE" => file.episode_number = Some(number),
                        "OP" => file.op_number = Some(number),
                        "ED" => file.ed_number = Some(number),
                        "SPECIAL" => file.special_number = Some(number),
                        _ => {}
                    }
                    app_state.persist_workspace()?;
                    return Ok(());
                }
            }
        }
    }

    Err("File not found".into())
}

#[tauri::command]
async fn reorder_files(
    volume_id: String,
    file_ids: Vec<String>,
    state: State<'_, AppStateType>,
) -> Result<(), String> {
    let mut app_state = state.lock();

    for volume in &mut app_state.current_volumes {
        if volume.id == volume_id {
            let mut reordered: Vec<MediaFile> = Vec::new();
            for id in &file_ids {
                if let Some(file) = volume.files.iter().find(|f| &f.id == id) {
                    reordered.push(file.clone());
                }
            }
            volume.files = reordered;
            app_state.persist_workspace()?;
            return Ok(());
        }
    }

    Err("Volume not found".into())
}

// ============================================================================
// Commands - Track Management
// ============================================================================

#[tauri::command]
async fn update_track(
    volume_id: String,
    file_id: String,
    track_id: i32,
    language: Option<String>,
    name: Option<String>,
    state: State<'_, AppStateType>,
) -> Result<(), String> {
    let mut app_state = state.lock();

    for volume in &mut app_state.current_volumes {
        if volume.id == volume_id {
            for file in &mut volume.files {
                if file.id == file_id {
                    for track in &mut file.tracks {
                        if track.id == track_id {
                            if let Some(lang) = language {
                                track.language = lang;
                            }
                            if let Some(n) = name {
                                track.name = n;
                            }
                            app_state.persist_workspace()?;
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    Err("Track not found".into())
}

#[tauri::command]
async fn apply_track_template(
    volume_id: String,
    category: String,
    template: Vec<Track>,
    state: State<'_, AppStateType>,
) -> Result<i32, String> {
    let mut app_state = state.lock();
    let mut applied_count = 0;

    for volume in &mut app_state.current_volumes {
        if volume.id == volume_id || volume_id == "all" {
            for file in &mut volume.files {
                if file.category == category {
                    // Apply template - match by track type and index
                    for (idx, template_track) in template.iter().enumerate() {
                        if idx < file.tracks.len()
                            && file.tracks[idx].track_type == template_track.track_type
                        {
                            file.tracks[idx].language = template_track.language.clone();
                            file.tracks[idx].name = template_track.name.clone();
                        }
                    }
                    applied_count += 1;
                }
            }
        }
    }

    app_state.persist_workspace()?;
    Ok(applied_count)
}

#[tauri::command]
async fn detect_track_mismatches(
    category: String,
    expected_track_count: usize,
    state: State<'_, AppStateType>,
) -> Result<Vec<MediaFile>, String> {
    let app_state = state.lock();
    let mut mismatches = Vec::new();

    for volume in &app_state.current_volumes {
        for file in &volume.files {
            if file.category == category && file.tracks.len() != expected_track_count {
                mismatches.push(file.clone());
            }
        }
    }

    Ok(mismatches)
}

// ============================================================================
// Commands - Project & Queue Management
// ============================================================================

#[tauri::command]
async fn create_project(
    output_name: String,
    output_path: String,
    state: State<'_, AppStateType>,
) -> Result<Project, String> {
    let mut app_state = state.lock();

    let project = Project {
        id: Uuid::new_v4().to_string(),
        output_name,
        output_path,
        volumes: app_state.current_volumes.clone(),
        track_templates: app_state.track_templates.clone(),
        status: ProjectStatus::Pending,
        progress: 0.0,
        current_file: None,
        created_at: chrono::Utc::now(),
        error: None,
    };

    app_state.queue.push(project.clone());
    app_state.persist_workspace()?;
    Ok(project)
}

#[tauri::command]
async fn remove_project(project_id: String, state: State<'_, AppStateType>) -> Result<(), String> {
    let mut app_state = state.lock();
    app_state.queue.retain(|p| p.id != project_id);
    app_state.persist_workspace()?;
    Ok(())
}

#[tauri::command]
async fn get_queue(state: State<'_, AppStateType>) -> Result<Vec<Project>, String> {
    let app_state = state.lock();
    app_state.ensure_workspace_available()?;
    Ok(app_state.queue.clone())
}

#[tauri::command]
async fn get_current_volumes(state: State<'_, AppStateType>) -> Result<Vec<Volume>, String> {
    let app_state = state.lock();
    app_state.ensure_workspace_available()?;
    Ok(app_state.current_volumes.clone())
}

#[tauri::command]
async fn clear_volumes(state: State<'_, AppStateType>) -> Result<(), String> {
    let mut app_state = state.lock();
    app_state.current_volumes.clear();
    app_state.persist_workspace()
}

#[tauri::command]
async fn reorder_queue(
    project_ids: Vec<String>,
    state: State<'_, AppStateType>,
) -> Result<(), String> {
    let mut app_state = state.lock();
    let mut reordered: Vec<Project> = Vec::new();

    for id in &project_ids {
        if let Some(project) = app_state.queue.iter().find(|p| &p.id == id) {
            reordered.push(project.clone());
        }
    }

    app_state.queue = reordered;
    app_state.persist_workspace()?;
    Ok(())
}

// ============================================================================
// Commands - Muxing
// ============================================================================

#[tauri::command]
async fn start_muxing(
    window: Window,
    state: State<'_, AppStateType>,
    muxer_state: State<'_, Arc<Mutex<Muxer>>>,
) -> Result<(), String> {
    let mut muxer = muxer_state.lock();

    if muxer.is_running {
        return Err("Muxer is already running".into());
    }

    {
        let mut app_state = state.lock();
        for project in &mut app_state.queue {
            if project.status == ProjectStatus::Paused {
                project.status = ProjectStatus::Pending;
            }
        }
        app_state.persist_workspace()?;
    }

    muxer.is_running = true;
    muxer.is_paused = false;

    // Clone what we need for the async task
    let state_clone = state.inner().clone();
    let muxer_clone = muxer_state.inner().clone();

    // Spawn the muxing task
    tauri::async_runtime::spawn(async move {
        run_mux_queue(window, state_clone, muxer_clone).await;
    });

    Ok(())
}

#[tauri::command]
async fn confirm_close(
    window: Window,
    state: State<'_, AppStateType>,
    muxer_state: State<'_, Arc<Mutex<Muxer>>>,
    close_approval: State<'_, Arc<AtomicBool>>,
) -> Result<(), String> {
    {
        let mut muxer = muxer_state.lock();
        muxer.is_running = false;
        muxer.is_paused = false;
    }
    {
        let mut app_state = state.lock();
        for project in &mut app_state.queue {
            if matches!(
                project.status,
                ProjectStatus::Pending | ProjectStatus::Processing
            ) {
                project.status = ProjectStatus::Paused;
            }
        }
        app_state.persist_workspace()?;
    }

    close_approval.store(true, Ordering::SeqCst);
    window
        .close()
        .map_err(|error| format!("Failed to close window: {}", error))
}

#[tauri::command]
async fn pause_muxing(muxer_state: State<'_, Arc<Mutex<Muxer>>>) -> Result<(), String> {
    let mut muxer = muxer_state.lock();
    muxer.is_paused = true;
    Ok(())
}

#[tauri::command]
async fn resume_muxing(muxer_state: State<'_, Arc<Mutex<Muxer>>>) -> Result<(), String> {
    let mut muxer = muxer_state.lock();
    muxer.is_paused = false;
    Ok(())
}

#[tauri::command]
async fn stop_muxing(muxer_state: State<'_, Arc<Mutex<Muxer>>>) -> Result<(), String> {
    let mut muxer = muxer_state.lock();
    muxer.is_running = false;
    muxer.is_paused = false;
    Ok(())
}

#[tauri::command]
async fn get_muxer_status(
    muxer_state: State<'_, Arc<Mutex<Muxer>>>,
) -> Result<MuxerStatus, String> {
    let muxer = muxer_state.lock();
    Ok(MuxerStatus {
        is_running: muxer.is_running,
        is_paused: muxer.is_paused,
    })
}

#[derive(Serialize)]
struct MuxerStatus {
    is_running: bool,
    is_paused: bool,
}

async fn run_mux_queue(window: Window, state: AppStateType, muxer_state: Arc<Mutex<Muxer>>) {
    loop {
        // Check if we should stop or wait (drop guard before any await)
        let (is_running, is_paused) = {
            let muxer = muxer_state.lock();
            (muxer.is_running, muxer.is_paused)
        };
        if !is_running {
            break;
        }
        if is_paused {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            continue;
        }

        // Get next pending project
        let project = {
            let mut app_state = state.lock();
            app_state
                .queue
                .iter_mut()
                .find(|p| matches!(p.status, ProjectStatus::Pending | ProjectStatus::Processing))
                .cloned()
        };

        match project {
            Some(project) => {
                // Update status to processing
                {
                    let mut app_state = state.lock();
                    if let Some(p) = app_state.queue.iter_mut().find(|p| p.id == project.id) {
                        p.status = ProjectStatus::Processing;
                    }
                    persist_from_mux(&window, &app_state);
                }

                // Emit progress event
                let _ = window.emit(
                    "mux-progress",
                    MuxProgress {
                        project_id: project.id.clone(),
                        progress: 0.0,
                        current_file: None,
                        status: "processing".into(),
                    },
                );

                // Process each volume
                let settings = {
                    let app_state = state.lock();
                    app_state.settings.clone()
                };

                let total_files: usize = project.volumes.iter().map(|v| v.files.len()).sum();
                let mut processed_files = 0;

                for volume in &project.volumes {
                    for file in &volume.files {
                        // Check pause/stop — drop guard before every await
                        loop {
                            let (is_running, is_paused) = {
                                let muxer = muxer_state.lock();
                                (muxer.is_running, muxer.is_paused)
                            };
                            if !is_running {
                                return;
                            }
                            if !is_paused {
                                break;
                            }
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        }

                        // Determine output path
                        let subfolder = match file.category.as_str() {
                            "EPISODE" => "Episode",
                            _ => "Extra",
                        };

                        let output_filename = generate_output_filename(file, &project.output_name);
                        let output_dir = PathBuf::from(&project.output_path)
                            .join(&project.output_name)
                            .join(subfolder);

                        // Create output directory
                        if let Err(e) = tokio::fs::create_dir_all(&output_dir).await {
                            eprintln!("Failed to create output directory: {}", e);
                            continue;
                        }

                        let output_path = output_dir.join(&output_filename);

                        // Update current file
                        {
                            let mut app_state = state.lock();
                            if let Some(p) = app_state.queue.iter_mut().find(|p| p.id == project.id)
                            {
                                p.current_file = Some(file.filename.clone());
                                p.progress = if total_files == 0 {
                                    0.0
                                } else {
                                    (processed_files as f64 / total_files as f64) * 100.0
                                };
                            }
                            persist_from_mux(&window, &app_state);
                        }

                        // Emit progress
                        let _ = window.emit(
                            "mux-progress",
                            MuxProgress {
                                project_id: project.id.clone(),
                                progress: (processed_files as f64 / total_files as f64) * 100.0,
                                current_file: Some(file.filename.clone()),
                                status: "processing".into(),
                            },
                        );

                        // Run mkvmerge
                        let job = MuxJob {
                            input_path: file.path.clone(),
                            output_path: output_path.to_string_lossy().to_string(),
                            chapter_path: file.chapter_file.clone(),
                            tracks: file.tracks.clone(),
                        };

                        if let Err(e) = Muxer::run_mkvmerge(&settings.mkvmerge_path, &job).await {
                            eprintln!("Muxing failed for {}: {}", file.filename, e);
                        }

                        processed_files += 1;
                    }
                }

                // Mark project as complete
                {
                    let mut app_state = state.lock();
                    if let Some(p) = app_state.queue.iter_mut().find(|p| p.id == project.id) {
                        p.status = ProjectStatus::Complete;
                        p.progress = 100.0;
                        p.current_file = None;
                    }
                    persist_from_mux(&window, &app_state);
                }

                let _ = window.emit(
                    "mux-progress",
                    MuxProgress {
                        project_id: project.id.clone(),
                        progress: 100.0,
                        current_file: None,
                        status: "complete".into(),
                    },
                );
            }
            None => {
                // No more projects, stop muxing
                let mut muxer = muxer_state.lock();
                muxer.is_running = false;
                break;
            }
        }
    }

    let _ = window.emit("mux-complete", ());
}

fn persist_from_mux(window: &Window, app_state: &AppState) {
    if let Err(error) = app_state.persist_workspace() {
        eprintln!("{}", error);
        let _ = window.emit("app-error", error);
    }
}

fn generate_output_filename(file: &MediaFile, show_name: &str) -> String {
    match file.category.as_str() {
        "EPISODE" => {
            let ep_num = file.episode_number.unwrap_or(1);
            format!("{} - E{:02}.mkv", show_name, ep_num)
        }
        "OP" => {
            let op_num = file.op_number.unwrap_or(1);
            format!("{} - OP{}.mkv", show_name, op_num)
        }
        "ED" => {
            let ed_num = file.ed_number.unwrap_or(1);
            format!("{} - ED{}.mkv", show_name, ed_num)
        }
        "SPECIAL" => {
            let sp_num = file.special_number.unwrap_or(1);
            format!("{} - SP{:02}.mkv", show_name, sp_num)
        }
        _ => {
            format!("{} - {}.mkv", show_name, file.filename.replace(".m2ts", ""))
        }
    }
}

// ============================================================================
// Commands - Settings
// ============================================================================

#[tauri::command]
async fn get_settings(state: State<'_, AppStateType>) -> Result<Settings, String> {
    let app_state = state.lock();
    Ok(app_state.settings.clone())
}

#[tauri::command]
async fn save_settings(settings: Settings, state: State<'_, AppStateType>) -> Result<(), String> {
    let mut app_state = state.lock();
    let config_dir = settings_config_dir()?;
    settings.save_to_config_dir(&config_dir)?;
    app_state.settings = settings;
    Ok(())
}

#[tauri::command]
async fn save_track_template(
    category: String,
    template: Vec<Track>,
    state: State<'_, AppStateType>,
) -> Result<(), String> {
    let mut app_state = state.lock();
    app_state.track_templates.insert(category, template);
    app_state.persist_workspace()
}

#[tauri::command]
async fn get_track_templates(
    state: State<'_, AppStateType>,
) -> Result<HashMap<String, Vec<Track>>, String> {
    let app_state = state.lock();
    app_state.ensure_workspace_available()?;
    Ok(app_state.track_templates.clone())
}

// ============================================================================
// Commands - Utilities
// ============================================================================

#[tauri::command]
async fn verify_mkvmerge(path: String) -> Result<String, String> {
    use tokio::process::Command;

    let output = Command::new(&path)
        .arg("--version")
        .output()
        .await
        .map_err(|e| format!("Failed to run mkvmerge: {}", e))?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout);
        Ok(version
            .lines()
            .next()
            .unwrap_or("Unknown version")
            .to_string())
    } else {
        Err("mkvmerge returned an error".into())
    }
}

#[tauri::command]
async fn get_media_info(path: String) -> Result<MediaFile, String> {
    let parser = BdmvParser::new();
    parser
        .get_media_info(&path)
        .await
        .map_err(|e| e.to_string())
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    let config_dir =
        settings_config_dir().expect("could not determine the platform config directory");
    let settings = Settings::load_from_config_dir(&config_dir).unwrap_or_else(|error| {
        eprintln!("{}; using default settings", error);
        Settings::default()
    });
    let mut restored_state = AppState::default();
    restored_state.settings = settings;
    if let Err(error) = restored_state.restore_workspace(&config_dir) {
        eprintln!("{}", error);
        restored_state.set_persistence_error(error, &config_dir);
    }
    let app_state: AppStateType = Arc::new(Mutex::new(restored_state));
    let muxer_state: Arc<Mutex<Muxer>> = Arc::new(Mutex::new(Muxer::default()));
    let close_approval = Arc::new(AtomicBool::new(false));

    tauri::Builder::default()
        .manage(app_state)
        .manage(muxer_state)
        .manage(close_approval.clone())
        .invoke_handler(tauri::generate_handler![
            // BDMV Loading
            load_bdmv,
            load_multiple_bdmvs,
            scan_for_volumes,
            // File Classification
            update_file_category,
            update_file_number,
            reorder_files,
            // Track Management
            update_track,
            apply_track_template,
            detect_track_mismatches,
            // Project & Queue
            create_project,
            remove_project,
            get_queue,
            get_current_volumes,
            clear_volumes,
            reorder_queue,
            // Muxing
            start_muxing,
            pause_muxing,
            resume_muxing,
            stop_muxing,
            get_muxer_status,
            confirm_close,
            // Settings
            get_settings,
            save_settings,
            save_track_template,
            get_track_templates,
            // Utilities
            verify_mkvmerge,
            get_media_info,
        ])
        .on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
                if close_approval.swap(false, Ordering::SeqCst) {
                    return;
                }
                let muxer_state = event.window().state::<Arc<Mutex<Muxer>>>();
                if muxer_state.lock().is_running {
                    api.prevent_close();
                    let _ = event.window().emit("mux-close-requested", ());
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn settings_config_dir() -> Result<PathBuf, String> {
    tauri::api::path::config_dir()
        .map(|path| path.join("bdmv-remuxer"))
        .ok_or_else(|| "could not determine the platform config directory".into())
}
