// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bdmv;
mod muxer;
mod state;
mod tool_error;

use crate::bdmv::{BdmvParser, MediaFile, Track, Volume};
use crate::muxer::{MuxJob, MuxProgress, Muxer};
use crate::state::{
    settings_directory_for_config_root, source_file_fingerprint, AppState, Project, ProjectStatus,
    Settings, SettingsRecoveryPreview,
};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
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
    let ffprobe_path = state.lock().settings.ffprobe_path.clone();
    let parser = BdmvParser::with_ffprobe(ffprobe_path);
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
    let ffprobe_path = state.lock().settings.ffprobe_path.clone();
    let parser = BdmvParser::with_ffprobe(ffprobe_path);
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
        completed_files: Vec::new(),
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
async fn retry_project(
    project_id: String,
    state: State<'_, AppStateType>,
) -> Result<Project, String> {
    let mut app_state = state.lock();
    let retried_project = prepare_project_retry(&mut app_state, &project_id)?;

    app_state.persist_workspace()?;
    Ok(retried_project)
}

#[tauri::command]
async fn update_project_output_path(
    project_id: String,
    output_path: String,
    state: State<'_, AppStateType>,
) -> Result<Project, String> {
    let mut app_state = state.lock();
    let updated_project =
        prepare_project_output_path_update(&mut app_state, &project_id, output_path)?;

    app_state.persist_workspace()?;
    Ok(updated_project)
}

fn prepare_project_output_path_update(
    app_state: &mut AppState,
    project_id: &str,
    output_path: String,
) -> Result<Project, String> {
    let project = app_state
        .queue
        .iter_mut()
        .find(|project| project.id == project_id)
        .ok_or_else(|| "Project was not found in the queue".to_string())?;

    if project.status != ProjectStatus::Error {
        return Err("Only projects with an error can change their output folder".into());
    }

    project.output_path = output_path;
    Ok(project.clone())
}

fn prepare_project_retry(app_state: &mut AppState, project_id: &str) -> Result<Project, String> {
    let project = app_state
        .queue
        .iter_mut()
        .find(|project| project.id == project_id)
        .ok_or_else(|| "Project was not found in the queue".to_string())?;

    if project.status != ProjectStatus::Error {
        return Err("Only projects with an error can be retried".into());
    }

    project.prepare_retry();
    Ok(project.clone())
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

    ensure_muxer_idle(&muxer)?;

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

fn ensure_muxer_idle(muxer: &Muxer) -> Result<(), String> {
    if muxer.is_running {
        Err("Muxer is already running".into())
    } else {
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
enum QueueAction {
    Stop,
    Wait,
    Process,
}

fn queue_action(is_running: bool, is_paused: bool, shutdown_requested: bool) -> QueueAction {
    if !is_running || shutdown_requested {
        QueueAction::Stop
    } else if is_paused {
        QueueAction::Wait
    } else {
        QueueAction::Process
    }
}

#[tauri::command]
async fn confirm_close(
    window: Window,
    state: State<'_, AppStateType>,
    muxer_state: State<'_, Arc<Mutex<Muxer>>>,
    close_approval: State<'_, Arc<AtomicBool>>,
) -> Result<(), String> {
    let wait_for_muxer = {
        // Keep the muxer lock while saving the paused queue so the worker cannot
        // start another file between the save and the shutdown request.
        let mut muxer = muxer_state.lock();
        let mut app_state = state.lock();
        prepare_close(&mut app_state, &mut muxer)?
    };

    while wait_for_muxer && muxer_state.lock().is_running {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    close_approval.store(true, Ordering::SeqCst);
    window
        .close()
        .map_err(|error| format!("Failed to close window: {}", error))
}

fn prepare_close(app_state: &mut AppState, muxer: &mut Muxer) -> Result<bool, String> {
    app_state.pause_incomplete_projects()?;
    if muxer.is_running {
        muxer.is_paused = false;
        muxer.shutdown_requested = true;
        Ok(true)
    } else {
        Ok(false)
    }
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

async fn remux_and_mark_file_completed<F, Fut>(
    project: &mut Project,
    source_path: &str,
    remux: F,
) -> Result<String, String>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let path = source_path.to_string();
    let source_fingerprint = tokio::task::spawn_blocking(move || source_file_fingerprint(&path))
        .await
        .map_err(|error| format!("Failed to verify source file before remux: {}", error))??;

    remux().await?;

    let path = source_path.to_string();
    let fingerprint_after_remux =
        tokio::task::spawn_blocking(move || source_file_fingerprint(&path))
            .await
            .map_err(|error| format!("Failed to verify source file after remux: {}", error))??;
    if fingerprint_after_remux != source_fingerprint {
        return Err(format!(
            "Source file changed during remux; retry this file: {}",
            source_path
        ));
    }

    project.mark_file_completed(source_path, source_fingerprint.clone());
    Ok(source_fingerprint)
}

async fn run_mux_queue(window: Window, state: AppStateType, muxer_state: Arc<Mutex<Muxer>>) {
    loop {
        // Check if we should stop or wait (drop guard before any await).
        let action = {
            let muxer = muxer_state.lock();
            queue_action(muxer.is_running, muxer.is_paused, muxer.shutdown_requested)
        };
        match action {
            QueueAction::Stop => break,
            QueueAction::Wait => {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                continue;
            }
            QueueAction::Process => {}
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
            Some(mut project) => {
                // Reconcile saved completion markers with the current source contents.
                let completed_source_paths: Vec<String> = project
                    .completed_files
                    .iter()
                    .map(|completed| completed.path.clone())
                    .collect();
                let current_fingerprints = tokio::task::spawn_blocking(move || {
                    completed_source_paths
                        .into_iter()
                        .filter_map(|path| {
                            source_file_fingerprint(&path)
                                .ok()
                                .map(|fingerprint| (path, fingerprint))
                        })
                        .collect::<HashMap<_, _>>()
                })
                .await
                .unwrap_or_default();
                project.retain_completed_files_matching(&current_fingerprints);

                // Update status to processing
                {
                    let mut app_state = state.lock();
                    if let Some(p) = app_state.queue.iter_mut().find(|p| p.id == project.id) {
                        p.status = ProjectStatus::Processing;
                        p.completed_files = project.completed_files.clone();
                        p.progress = project.progress_for_completed_files();
                    }
                    persist_from_mux(&window, &app_state);
                }

                // Emit progress event
                let _ = window.emit(
                    "mux-progress",
                    MuxProgress {
                        project_id: project.id.clone(),
                        progress: project.progress_for_completed_files(),
                        current_file: None,
                        status: "processing".into(),
                        error: None,
                    },
                );

                // Process each volume
                let settings = {
                    let app_state = state.lock();
                    app_state.settings.clone()
                };

                let files: Vec<_> = project
                    .volumes
                    .iter()
                    .flat_map(|volume| volume.files.iter().cloned())
                    .collect();
                let total_files = files.len();
                let mut processed_files = project.completed_file_count();
                let mut failed_files = Vec::new();
                let first_unfinished_index = project.first_unfinished_file_index();

                for (file_index, file) in files.into_iter().enumerate() {
                    if first_unfinished_index
                        .map(|first_unfinished| file_index < first_unfinished)
                        .unwrap_or(true)
                    {
                        continue;
                    }
                    if project.is_file_completed(&file.path) {
                        continue;
                    }

                    // Check pause/stop/shutdown — drop guard before every await.
                    // Shutdown waits for the current file, then stops at this boundary.
                    loop {
                        let (is_running, is_paused, shutdown_requested) = {
                            let muxer = muxer_state.lock();
                            (muxer.is_running, muxer.is_paused, muxer.shutdown_requested)
                        };
                        if !is_running {
                            return;
                        }
                        if shutdown_requested {
                            break;
                        }
                        if !is_paused {
                            break;
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                    if muxer_state.lock().shutdown_requested {
                        break;
                    }

                    // Determine output path
                    let subfolder = match file.category.as_str() {
                        "EPISODE" => "Episode",
                        _ => "Extra",
                    };

                    let output_filename = generate_output_filename(&file, &project.output_name);
                    let output_dir = PathBuf::from(&project.output_path)
                        .join(&project.output_name)
                        .join(subfolder);

                    // Create output directory
                    if let Err(e) = tokio::fs::create_dir_all(&output_dir).await {
                        eprintln!("Failed to create output directory: {}", e);
                        failed_files.push(format!("{}: {}", file.filename, e));
                        continue;
                    }

                    let output_path = output_dir.join(&output_filename);

                    // Update current file
                    {
                        let mut app_state = state.lock();
                        if let Some(p) = app_state.queue.iter_mut().find(|p| p.id == project.id) {
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
                            error: None,
                        },
                    );

                    // Run mkvmerge
                    let job = MuxJob {
                        input_path: file.path.clone(),
                        output_path: output_path.to_string_lossy().to_string(),
                        chapter_path: file.chapter_file.clone(),
                        tracks: file.tracks.clone(),
                    };

                    let muxer_path = settings.mkvmerge_path.clone();
                    match remux_and_mark_file_completed(&mut project, &file.path, || async {
                        Muxer::run_mkvmerge(&muxer_path, &job).await
                    })
                    .await
                    {
                        Ok(source_fingerprint) => {
                            processed_files += 1;
                            let mut app_state = state.lock();
                            if let Some(p) = app_state.queue.iter_mut().find(|p| p.id == project.id)
                            {
                                p.mark_file_completed(&file.path, source_fingerprint);
                                p.progress = if total_files == 0 {
                                    0.0
                                } else {
                                    (processed_files as f64 / total_files as f64) * 100.0
                                };
                                p.current_file = None;
                            }
                            persist_from_mux(&window, &app_state);
                            let _ = window.emit(
                                "mux-progress",
                                MuxProgress {
                                    project_id: project.id.clone(),
                                    progress: if total_files == 0 {
                                        0.0
                                    } else {
                                        (processed_files as f64 / total_files as f64) * 100.0
                                    },
                                    current_file: None,
                                    status: "processing".into(),
                                    error: None,
                                },
                            );
                        }
                        Err(error) => {
                            eprintln!("Muxing failed for {}: {}", file.filename, error);
                            let _ =
                                window.emit("app-error", format!("{}: {}", file.filename, error));
                            failed_files.push(format!("{}: {}", file.filename, error));
                        }
                    }
                }

                let shutdown_requested = muxer_state.lock().shutdown_requested;
                if shutdown_requested && failed_files.is_empty() && processed_files < total_files {
                    let progress = if total_files == 0 {
                        0.0
                    } else {
                        (processed_files as f64 / total_files as f64) * 100.0
                    };
                    {
                        let mut app_state = state.lock();
                        if let Some(p) = app_state.queue.iter_mut().find(|p| p.id == project.id) {
                            p.status = ProjectStatus::Paused;
                            p.progress = progress;
                            p.current_file = None;
                            p.error = None;
                        }
                        persist_from_mux(&window, &app_state);
                    }
                    let _ = window.emit(
                        "mux-progress",
                        MuxProgress {
                            project_id: project.id.clone(),
                            progress,
                            current_file: None,
                            status: "paused".into(),
                            error: None,
                        },
                    );
                    break;
                }

                let error_message = if failed_files.is_empty() && processed_files == total_files {
                    None
                } else {
                    Some(format!(
                        "Some source files could not be processed: {}",
                        failed_files.join("; ")
                    ))
                };

                // Mark the project complete only when all its source files succeeded.
                {
                    let mut app_state = state.lock();
                    if let Some(p) = app_state.queue.iter_mut().find(|p| p.id == project.id) {
                        if error_message.is_none() {
                            p.status = ProjectStatus::Complete;
                            p.progress = 100.0;
                            p.current_file = None;
                            p.error = None;
                        } else {
                            p.status = ProjectStatus::Error;
                            p.progress = if total_files == 0 {
                                0.0
                            } else {
                                (processed_files as f64 / total_files as f64) * 100.0
                            };
                            p.current_file = None;
                            p.error = error_message.clone();
                        }
                    }
                    persist_from_mux(&window, &app_state);
                }

                let (final_status, final_progress) = if error_message.is_none() {
                    ("complete", 100.0)
                } else {
                    (
                        "error",
                        if total_files == 0 {
                            0.0
                        } else {
                            (processed_files as f64 / total_files as f64) * 100.0
                        },
                    )
                };
                let _ = window.emit(
                    "mux-progress",
                    MuxProgress {
                        project_id: project.id.clone(),
                        progress: final_progress,
                        current_file: None,
                        status: final_status.into(),
                        error: error_message,
                    },
                );
            }
            None => {
                // No more projects.
                break;
            }
        }
    }

    {
        let mut muxer = muxer_state.lock();
        muxer.is_running = false;
        muxer.is_paused = false;
        muxer.shutdown_requested = false;
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
async fn get_settings(state: State<'_, AppStateType>) -> Result<SettingsResponse, String> {
    let app_state = state.lock();
    Ok(SettingsResponse {
        settings: app_state.settings.clone(),
        recovery_warning: app_state.settings_recovery_warning().map(str::to_string),
        recovery_backup_available: app_state.settings_recovery_backup_path().is_some(),
        settings_save_allowed: app_state.settings_can_save(),
    })
}

#[derive(Serialize)]
struct SettingsResponse {
    settings: Settings,
    recovery_warning: Option<String>,
    recovery_backup_available: bool,
    settings_save_allowed: bool,
}

#[tauri::command]
async fn open_settings_recovery_backup(
    window: Window,
    state: State<'_, AppStateType>,
) -> Result<(), String> {
    let backup_path = settings_recovery_backup_path(&state)?;

    open_recovery_backup(&window, &backup_path)
}

fn settings_recovery_backup_path(state: &AppStateType) -> Result<PathBuf, String> {
    state
        .lock()
        .settings_recovery_backup_path()
        .map(PathBuf::from)
        .ok_or_else(|| "There is no recovered preferences backup to open.".to_string())
}

fn open_recovery_backup(window: &Window, backup_path: &Path) -> Result<(), String> {
    open_recovery_backup_with(backup_path, |path| {
        let path = path.to_string_lossy();
        tauri::api::shell::open(&window.shell_scope(), path.as_ref(), None)
            .map_err(|error| error.to_string())
    })
}

fn open_recovery_backup_with(
    backup_path: &Path,
    opener: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    if !backup_path.is_file() {
        return Err(format!(
            "The recovered preferences backup is no longer available at {}.",
            backup_path.display()
        ));
    }

    opener(backup_path).map_err(|error| {
        format!(
            "Could not open the recovered preferences backup at {}: {}",
            backup_path.display(),
            error
        )
    })
}

#[tauri::command]
async fn preview_settings_recovery_backup(
    state: State<'_, AppStateType>,
) -> Result<SettingsRecoveryPreview, String> {
    let backup_path = state
        .lock()
        .settings_recovery_backup_path()
        .map(PathBuf::from)
        .ok_or_else(|| "There is no recovered preferences backup to restore.".to_string())?;

    Settings::preview_recovery_backup(&backup_path)
}

#[tauri::command]
async fn restore_settings_recovery_backup(
    expected_source_hash: String,
    state: State<'_, AppStateType>,
) -> Result<SettingsSaveResponse, String> {
    let backup_path = {
        let app_state = state.lock();
        app_state.ensure_settings_savable()?;
        app_state
            .settings_recovery_backup_path()
            .map(PathBuf::from)
            .ok_or_else(|| "There is no recovered preferences backup to restore.".to_string())?
    };
    let preview = Settings::preview_recovery_backup(&backup_path)?;
    if preview.source_hash != expected_source_hash {
        return Err(
            "The recovered preferences backup changed after it was reviewed. Preview it again before restoring."
                .to_string(),
        );
    }
    let config_dir = settings_config_dir()?;
    let saved_settings = preview.settings.save_to_config_dir(&config_dir)?;

    let mut app_state = state.lock();
    app_state.settings = saved_settings.clone();
    let warning = saved_settings.unrecognized_fields_warning();
    app_state.set_settings_recovery(warning.clone(), None, true);
    Ok(SettingsSaveResponse {
        settings: saved_settings,
        recovery_warning: warning,
    })
}

#[tauri::command]
async fn save_settings(
    settings: Settings,
    state: State<'_, AppStateType>,
) -> Result<SettingsSaveResponse, String> {
    let mut app_state = state.lock();
    app_state.ensure_settings_savable()?;
    let config_dir = settings_config_dir()?;
    let saved_settings = settings.save_to_config_dir(&config_dir)?;
    let warning = saved_settings.unrecognized_fields_warning();
    app_state.settings = saved_settings.clone();
    app_state.set_settings_recovery(warning.clone(), None, true);
    Ok(SettingsSaveResponse {
        settings: saved_settings,
        recovery_warning: warning,
    })
}

#[derive(Serialize)]
struct SettingsSaveResponse {
    settings: Settings,
    recovery_warning: Option<String>,
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
        .map_err(|e| {
            tool_error::format_tool_launch_error(
                "mkvmerge",
                &path,
                "MKVToolNix Path",
                "MKVToolNix",
                e,
            )
        })?;

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
async fn get_media_info(path: String, state: State<'_, AppStateType>) -> Result<MediaFile, String> {
    let ffprobe_path = state.lock().settings.ffprobe_path.clone();
    let parser = BdmvParser::with_ffprobe(ffprobe_path);
    parser
        .get_media_info(&path)
        .await
        .map_err(|e| e.to_string())
}

fn should_intercept_close(is_running: bool, close_was_approved: bool) -> bool {
    is_running && !close_was_approved
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--smoke-remux") {
        let result = run_remux_smoke(&args);
        let report = match &result {
            Ok(report) => report.clone(),
            Err(error) => format!("Installed app remux smoke failed: {}\n", error),
        };

        if let Some(log_path) = args.get(5) {
            if let Err(error) = std::fs::write(log_path, &report) {
                eprintln!(
                    "Could not write remux smoke report to {}: {}\n{}",
                    log_path, error, report
                );
                std::process::exit(2);
            }
        }
        eprint!("{}", report);
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }

    let recovery_smoke = if args.get(1).map(String::as_str) == Some("--smoke-settings-recovery") {
        if args.len() != 4 {
            eprintln!(
                "Usage: bdmv-remuxer --smoke-settings-recovery <config-directory> <report.log>"
            );
            std::process::exit(2);
        }
        Some((PathBuf::from(&args[2]), PathBuf::from(&args[3])))
    } else {
        None
    };
    let config_dir = match recovery_smoke.as_ref() {
        Some((config_dir, _)) => config_dir.clone(),
        None => settings_config_dir().expect("could not determine the platform config directory"),
    };
    let settings_outcome = Settings::load_with_recovery(&config_dir);
    if let Some(warning) = &settings_outcome.recovery_warning {
        eprintln!("{}", warning);
    }
    let mut restored_state = AppState::default();
    restored_state.settings = settings_outcome.settings;
    restored_state.set_settings_recovery(
        settings_outcome.recovery_warning,
        settings_outcome.recovery_backup_path,
        settings_outcome.can_save,
    );
    if recovery_smoke.is_none() {
        if let Err(error) = restored_state.restore_workspace(&config_dir) {
            eprintln!("{}", error);
            restored_state.set_persistence_error(error, &config_dir);
        }
    }
    let app_state: AppStateType = Arc::new(Mutex::new(restored_state));
    let muxer_state: Arc<Mutex<Muxer>> = Arc::new(Mutex::new(Muxer::default()));
    let close_approval = Arc::new(AtomicBool::new(false));
    let recovery_smoke_report_path = recovery_smoke.map(|(_, report_path)| report_path);

    tauri::Builder::default()
        .manage(app_state)
        .manage(muxer_state)
        .manage(close_approval.clone())
        .setup(move |app| {
            if let Some(report_path) = recovery_smoke_report_path {
                let result = (|| -> Result<String, String> {
                    let state = app.state::<AppStateType>();
                    let backup_path = settings_recovery_backup_path(&state)?;
                    let window = app
                        .get_window("main")
                        .ok_or_else(|| "The app's main window is unavailable.".to_string())?;
                    let original_contents = std::fs::read(&backup_path).map_err(|error| {
                        format!(
                            "Could not read the recovered preferences backup at {}: {}",
                            backup_path.display(),
                            error
                        )
                    })?;
                    open_recovery_backup(&window, &backup_path)?;
                    let opened_contents = std::fs::read(&backup_path).map_err(|error| {
                        format!(
                            "Could not verify the recovered preferences backup at {}: {}",
                            backup_path.display(),
                            error
                        )
                    })?;
                    if opened_contents != original_contents {
                        return Err(format!(
                            "Opening the recovered preferences backup changed its contents: {}",
                            backup_path.display()
                        ));
                    }
                    Ok(format!(
                        "Opened recovered preferences backup without changing it:\n{}\n",
                        backup_path.display()
                    ))
                })();

                let (exit_code, report) = match result {
                    Ok(report) => (0, report),
                    Err(error) => (
                        1,
                        format!("Installed app recovery smoke failed: {}\n", error),
                    ),
                };
                if let Err(error) = std::fs::write(&report_path, &report) {
                    eprintln!(
                        "Could not write recovery smoke report to {}: {}\n{}",
                        report_path.display(),
                        error,
                        report
                    );
                    app.handle().exit(2);
                } else {
                    eprint!("{}", report);
                    app.handle().exit(exit_code);
                }
            }
            Ok(())
        })
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
            update_project_output_path,
            retry_project,
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
            open_settings_recovery_backup,
            preview_settings_recovery_backup,
            restore_settings_recovery_backup,
            save_settings,
            save_track_template,
            get_track_templates,
            // Utilities
            verify_mkvmerge,
            get_media_info,
        ])
        .on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
                let close_was_approved = close_approval.swap(false, Ordering::SeqCst);
                let muxer_state = event.window().state::<Arc<Mutex<Muxer>>>();
                if should_intercept_close(muxer_state.lock().is_running, close_was_approved) {
                    api.prevent_close();
                    let _ = event.window().emit("mux-close-requested", ());
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn run_remux_smoke(args: &[String]) -> Result<String, String> {
    if args.len() != 6 {
        return Err(
            "Usage: bdmv-remuxer --smoke-remux <mkvmerge> <input.mkv> <output.mkv> <report.log>"
                .into(),
        );
    }

    let job = MuxJob {
        input_path: args[3].clone(),
        output_path: args[4].clone(),
        chapter_path: None,
        tracks: Vec::new(),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Could not start remux runtime: {}", error))?;
    runtime.block_on(Muxer::run_mkvmerge(&args[2], &job))?;

    let output = std::fs::metadata(&args[4]).map_err(|error| {
        format!(
            "Remux reported success but output {} is missing: {}",
            args[4], error
        )
    })?;
    if output.len() == 0 {
        return Err(format!("Remux created an empty output file: {}", args[4]));
    }

    Ok(format!(
        "Installed app remux smoke passed.\nInput: {}\nOutput: {}\nOutput size: {} bytes\n",
        args[3],
        args[4],
        output.len()
    ))
}

#[cfg(test)]
mod recovery_open_tests {
    use super::open_recovery_backup_with;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temporary_backup(contents: &[u8]) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "bdmv-remuxer-recovery-open-test-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("settings.json.corrupt.1");
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn recovery_open_passes_the_preserved_backup_path_to_the_shell_launcher() {
        let original = b"{ damaged preferences";
        let path = temporary_backup(original);
        let mut opened_path = None;

        open_recovery_backup_with(&path, |path| {
            opened_path = Some(path.to_path_buf());
            Ok(())
        })
        .unwrap();

        assert_eq!(opened_path.as_deref(), Some(path.as_path()));
        assert_eq!(fs::read(&path).unwrap(), original);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn recovery_open_reports_a_missing_backup_with_its_path() {
        let path =
            std::env::temp_dir().join(format!("bdmv-remuxer-missing-{}.corrupt", Uuid::new_v4()));
        let error = open_recovery_backup_with(&path, |_| {
            panic!("the shell launcher must not run for a missing backup")
        })
        .unwrap_err();

        assert!(error.contains("backup is no longer available"));
        assert!(error.contains(&path.display().to_string()));
    }

    #[test]
    fn recovery_open_explains_operating_system_launch_failures() {
        let path = temporary_backup(b"preserved settings");
        let error =
            open_recovery_backup_with(&path, |_| Err("access was denied".to_string())).unwrap_err();

        assert!(error.contains("Could not open the recovered preferences backup"));
        assert!(error.contains(&path.display().to_string()));
        assert!(error.contains("access was denied"));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}

#[cfg(test)]
mod close_tests {
    use super::{prepare_close, should_intercept_close, Project, ProjectStatus};
    use crate::muxer::Muxer;
    use crate::state::AppState;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};
    use uuid::Uuid;

    #[test]
    fn close_is_intercepted_only_while_muxing_without_prior_approval() {
        assert!(!should_intercept_close(false, false));
        assert!(should_intercept_close(true, false));
        assert!(!should_intercept_close(true, true));
    }

    #[test]
    fn failed_workspace_save_keeps_muxer_running_and_retry_persists_paused_queue() {
        let config_dir =
            std::env::temp_dir().join(format!("bdmv-remuxer-close-test-{}", Uuid::new_v4()));
        let mut app_state = AppState::default();
        app_state.restore_workspace(&config_dir).unwrap();
        app_state.queue.push(Project {
            id: "queued-project".into(),
            output_name: "Queued project".into(),
            output_path: "/output".into(),
            volumes: Vec::new(),
            track_templates: HashMap::new(),
            status: ProjectStatus::Pending,
            progress: 0.0,
            current_file: None,
            completed_files: Vec::new(),
            created_at: Utc::now(),
            error: None,
        });

        fs::remove_dir_all(&config_dir).unwrap();
        fs::write(&config_dir, b"block workspace directory creation").unwrap();

        let mut muxer = Muxer {
            is_running: true,
            is_paused: false,
            shutdown_requested: false,
        };
        let close_approval = AtomicBool::new(false);

        let error = prepare_close(&mut app_state, &mut muxer).unwrap_err();

        assert!(error.contains("Failed to create workspace directory"));
        assert!(should_intercept_close(
            muxer.is_running,
            close_approval.load(Ordering::SeqCst)
        ));
        assert!(!muxer.shutdown_requested);
        assert_eq!(app_state.queue[0].status, ProjectStatus::Paused);

        fs::remove_file(&config_dir).unwrap();
        fs::create_dir_all(&config_dir).unwrap();

        assert!(prepare_close(&mut app_state, &mut muxer).unwrap());
        assert!(muxer.shutdown_requested);
        assert!(!close_approval.load(Ordering::SeqCst));
        let saved_workspace: serde_json::Value =
            serde_json::from_slice(&fs::read(config_dir.join("workspace.json")).unwrap()).unwrap();
        assert_eq!(saved_workspace["queue"][0]["status"], "paused");

        fs::remove_dir_all(config_dir).unwrap();
    }
}

#[cfg(test)]
mod queue_retry_tests {
    use super::{
        ensure_muxer_idle, prepare_project_output_path_update, prepare_project_retry, queue_action,
        QueueAction,
    };
    use crate::bdmv::{MediaFile, Volume};
    use crate::muxer::Muxer;
    use crate::state::{AppState, CompletedFile, Project, ProjectStatus};
    use chrono::Utc;
    use std::collections::HashMap;

    fn retry_state() -> AppState {
        let successful_path = "/disc/BDMV/STREAM/00001.m2ts";
        let remaining_path = "/disc/BDMV/STREAM/00002.m2ts";
        let make_file = |filename: &str, path: &str| MediaFile {
            id: filename.into(),
            filename: filename.into(),
            path: path.into(),
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
        let mut state = AppState::default();
        state.queue.push(Project {
            id: "active-project".into(),
            output_name: "Active".into(),
            output_path: "/output".into(),
            volumes: Vec::new(),
            track_templates: HashMap::new(),
            status: ProjectStatus::Processing,
            progress: 0.0,
            current_file: Some("active.m2ts".into()),
            completed_files: Vec::new(),
            created_at: Utc::now(),
            error: None,
        });
        state.queue.push(Project {
            id: "failed-project".into(),
            output_name: "Retry me".into(),
            output_path: "/output".into(),
            volumes: vec![Volume {
                id: "volume-1".into(),
                name: "Disc 1".into(),
                volume_number: 1,
                files: vec![
                    make_file("00001.m2ts", successful_path),
                    make_file("00002.m2ts", remaining_path),
                ],
                probe_failures: Vec::new(),
            }],
            track_templates: HashMap::new(),
            status: ProjectStatus::Error,
            progress: 50.0,
            current_file: Some("00002.m2ts".into()),
            completed_files: vec![CompletedFile {
                path: successful_path.into(),
                fingerprint: Some("successful-file-fingerprint".into()),
            }],
            created_at: Utc::now(),
            error: Some("mux failed".into()),
        });
        state
    }

    fn assert_retry_keeps_successful_files(state: &mut AppState) {
        let retried = prepare_project_retry(state, "failed-project").unwrap();
        assert_eq!(retried.status, ProjectStatus::Pending);
        assert_eq!(retried.current_file, None);
        assert_eq!(retried.error, None);
        assert_eq!(
            retried.completed_files,
            vec![CompletedFile {
                path: "/disc/BDMV/STREAM/00001.m2ts".into(),
                fingerprint: Some("successful-file-fingerprint".into()),
            }]
        );
        assert_eq!(
            state.queue[1].completed_files,
            retried.completed_files,
            "the queued project must retain the same successful-file markers"
        );
        assert_eq!(state.queue[1].progress, 50.0);
    }

    #[test]
    fn changing_failed_project_output_keeps_completed_markers_and_retry_starts_at_unfinished_file() {
        let mut state = retry_state();
        let completed_files = state.queue[1].completed_files.clone();
        let active_output_path = state.queue[0].output_path.clone();
        assert_eq!(
            prepare_project_output_path_update(&mut state, "active-project", "/wrong".into())
                .unwrap_err(),
            "Only projects with an error can change their output folder"
        );
        assert_eq!(state.queue[0].output_path, active_output_path);

        let updated =
            prepare_project_output_path_update(&mut state, "failed-project", "/new-output".into())
                .unwrap();

        assert_eq!(updated.output_path, "/new-output");
        assert_eq!(updated.status, ProjectStatus::Error);
        assert_eq!(updated.completed_files, completed_files);
        assert_eq!(state.queue[1].output_path, "/new-output");
        assert_eq!(state.queue[1].completed_files, completed_files);

        let retried = prepare_project_retry(&mut state, "failed-project").unwrap();
        assert_eq!(retried.output_path, "/new-output");
        assert_eq!(retried.first_unfinished_file_index(), Some(1));
    }

    #[test]
    fn retry_while_another_project_is_running_does_not_start_a_second_runner() {
        let mut state = retry_state();
        let mut muxer = Muxer {
            is_running: true,
            is_paused: false,
            shutdown_requested: false,
        };

        assert_retry_keeps_successful_files(&mut state);
        assert_eq!(
            queue_action(muxer.is_running, muxer.is_paused, muxer.shutdown_requested),
            QueueAction::Process
        );
        assert_eq!(
            ensure_muxer_idle(&muxer).unwrap_err(),
            "Muxer is already running"
        );
        muxer.is_running = false;
        assert_eq!(ensure_muxer_idle(&muxer), Ok(()));
    }

    #[test]
    fn retry_while_paused_waits_for_resume_and_preserves_successful_files() {
        let mut state = retry_state();
        let mut muxer = Muxer {
            is_running: true,
            is_paused: true,
            shutdown_requested: false,
        };

        assert_retry_keeps_successful_files(&mut state);
        assert_eq!(
            queue_action(muxer.is_running, muxer.is_paused, muxer.shutdown_requested),
            QueueAction::Wait
        );
        assert_eq!(
            ensure_muxer_idle(&muxer).unwrap_err(),
            "Muxer is already running"
        );

        muxer.is_paused = false;
        assert_eq!(
            queue_action(muxer.is_running, muxer.is_paused, muxer.shutdown_requested),
            QueueAction::Process
        );
        assert_eq!(
            ensure_muxer_idle(&muxer).unwrap_err(),
            "Muxer is already running"
        );
    }
}

#[cfg(test)]
mod remux_source_change_tests {
    use super::remux_and_mark_file_completed;
    use crate::state::{Project, ProjectStatus};
    use chrono::Utc;
    use std::collections::HashMap;
    use std::fs;
    use uuid::Uuid;

    #[tokio::test]
    async fn source_changed_during_remux_is_not_marked_complete() {
        let directory = std::env::temp_dir().join(format!(
            "bdmv-remuxer-source-change-test-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("source.m2ts").to_string_lossy().to_string();
        fs::write(&source_path, b"original source contents").unwrap();
        let mut project = Project {
            id: "source-change-project".into(),
            output_name: "source-change".into(),
            output_path: directory.to_string_lossy().to_string(),
            volumes: Vec::new(),
            track_templates: HashMap::new(),
            status: ProjectStatus::Processing,
            progress: 0.0,
            current_file: None,
            completed_files: Vec::new(),
            created_at: Utc::now(),
            error: None,
        };

        let result = remux_and_mark_file_completed(&mut project, &source_path, || async {
            tokio::fs::write(&source_path, b"changed during remux")
                .await
                .map_err(|error| error.to_string())
        })
        .await;

        let error = result.expect_err("changed source must fail verification");
        assert!(error.contains("changed during remux"));
        assert!(!project.is_file_completed(&source_path));
        fs::remove_dir_all(directory).unwrap();
    }
}

fn settings_config_dir() -> Result<PathBuf, String> {
    tauri::api::path::config_dir()
        .map(|path| settings_directory_for_config_root(&path))
        .ok_or_else(|| "could not determine the platform config directory".into())
}
