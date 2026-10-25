import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/tauri';
import { confirm, open } from '@tauri-apps/api/dialog';
import { listen } from '@tauri-apps/api/event';
import { retryProjectWithQueue } from './retryProject.js';
import type {
  Volume,
  Project,
  Settings,
  SettingsRecoveryPreview,
  Track,
  FileCategory,
} from './types';
import {
  getMuxCloseConfirmation,
  handleMuxCloseRequested,
} from './muxCloseConfirmation.js';
import {
  getRecoveryWarningDismissUpdate,
  getSettingsLoadUpdate,
  getSettingsSaveFailureUpdate,
  getSettingsSaveSuccessUpdate,
} from './settingsRecoveryNotice.js';

interface MuxerStatus {
  is_running: boolean;
  is_paused: boolean;
}

interface MuxProgress {
  project_id: string;
  progress: number;
  current_file?: string;
  resume_reason?: string | null;
  status: string;
  error?: string | null;
}

interface AppStore {
  // State
  volumes: Volume[];
  queue: Project[];
  settings: Settings | null;
  recoveryWarning: string | null;
  recoveryBackupAvailable: boolean;
  settingsSaveAllowed: boolean;
  recoveryRetryAvailable: boolean;
  isRetryingRecoveryCopy: boolean;
  recoveryBackupOpenError: string | null;
  recoveryRestorePreview: SettingsRecoveryPreview | null;
  recoveryRestoreError: string | null;
  isPreparingRecoveryRestore: boolean;
  isRestoringRecoveryBackup: boolean;
  trackTemplates: Record<string, Track[]>;
  selectedFileId: string | null;
  expandedVolumes: Record<string, boolean>;
  muxerStatus: MuxerStatus;
  isLoading: boolean;
  error: string | null;
  outputName: string;
  outputPath: string;
  retryingProbeFailures: Record<string, boolean>;
  retryingProbeBatches: Record<string, boolean>;
  retryingProjects: Record<string, boolean>;

  // Actions
  loadBdmv: () => Promise<void>;
  loadMultipleBdmvs: () => Promise<void>;
  retryProbeFailure: (volumeId: string, path: string) => Promise<void>;
  retryAllProbeFailures: (volumeId: string) => Promise<void>;
  clearVolumes: () => Promise<void>;
  selectFile: (fileId: string) => void;
  toggleVolumeExpanded: (volumeId: string) => void;
  updateFileCategory: (volumeId: string, fileId: string, category: FileCategory) => Promise<void>;
  updateTrack: (volumeId: string, fileId: string, trackId: number, language: string, name: string) => Promise<void>;
  createProject: () => Promise<void>;
  removeProject: (projectId: string) => Promise<void>;
  changeProjectOutputPath: (projectId: string) => Promise<void>;
  retryProject: (projectId: string) => Promise<void>;
  startMuxing: () => Promise<void>;
  pauseMuxing: () => Promise<void>;
  resumeMuxing: () => Promise<void>;
  stopMuxing: () => Promise<void>;
  loadSettings: () => Promise<void>;
  retryRecoveryCopy: () => Promise<boolean>;
  openRecoveryBackup: () => Promise<void>;
  previewRecoveryBackup: () => Promise<void>;
  restoreRecoveryBackup: () => Promise<void>;
  dismissRecoveryRestorePreview: () => void;
  dismissRecoveryWarning: () => void;
  dismissRecoveryBackupOpenError: () => void;
  saveSettings: (settings: Settings) => Promise<boolean>;
  loadQueue: () => Promise<void>;
  loadCurrentVolumes: () => Promise<void>;
  loadTrackTemplates: () => Promise<void>;
  saveTrackTemplate: (category: FileCategory, template: Track[]) => Promise<void>;
  setOutputName: (name: string) => void;
  selectOutputPath: () => Promise<void>;
  setupEventListeners: () => void;
}

export const useAppStore = create<AppStore>((set, get) => ({
  volumes: [],
  queue: [],
  settings: null,
  recoveryWarning: null,
  recoveryBackupAvailable: false,
  settingsSaveAllowed: true,
  recoveryRetryAvailable: false,
  isRetryingRecoveryCopy: false,
  recoveryBackupOpenError: null,
  recoveryRestorePreview: null,
  recoveryRestoreError: null,
  isPreparingRecoveryRestore: false,
  isRestoringRecoveryBackup: false,
  trackTemplates: {},
  selectedFileId: null,
  expandedVolumes: {},
  muxerStatus: { is_running: false, is_paused: false },
  isLoading: false,
  error: null,
  outputName: '',
  outputPath: '',
  retryingProbeFailures: {},
  retryingProbeBatches: {},
  retryingProjects: {},

  loadBdmv: async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select BDMV folder',
      });
      if (!selected || Array.isArray(selected)) return;

      set({ isLoading: true, error: null });
      const volume = await invoke<Volume>('load_bdmv', { path: selected });
      set(state => ({
        volumes: [...state.volumes, volume],
        expandedVolumes: { ...state.expandedVolumes, [volume.id]: true },
        isLoading: false,
      }));
    } catch (e) {
      set({ error: String(e), isLoading: false });
    }
  },

  loadMultipleBdmvs: async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: true,
        title: 'Select BDMV folders',
      });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      if (paths.length === 0) return;

      set({ isLoading: true, error: null });
      const volumes = await invoke<Volume[]>('load_multiple_bdmvs', { paths });
      const expanded: Record<string, boolean> = {};
      volumes.forEach(v => { expanded[v.id] = true; });
      set(state => ({
        volumes: [...state.volumes, ...volumes],
        expandedVolumes: { ...state.expandedVolumes, ...expanded },
        isLoading: false,
      }));
    } catch (e) {
      set({ error: String(e), isLoading: false });
    }
  },

  retryProbeFailure: async (volumeId, path) => {
    const retryKey = `${volumeId}:${path}`;
    if (Object.keys(get().retryingProbeFailures).some(key => key.startsWith(`${volumeId}:`))) return;
    set(state => ({
      retryingProbeFailures: { ...state.retryingProbeFailures, [retryKey]: true },
    }));

    try {
      const result = await invoke<{
        volume_id: string;
        path: string;
        file: Volume['files'][number] | null;
        failure: Volume['probe_failures'][number] | null;
      }>('retry_probe_failure', { volumeId, path });
      set(state => ({
        volumes: state.volumes.map(volume => {
          if (volume.id !== result.volume_id) return volume;
          const files = result.file && !volume.files.some(file => file.path === result.path)
            ? [...volume.files, result.file]
            : volume.files;
          const probeFailures = result.file
            ? volume.probe_failures.filter(failure => failure.path !== result.path)
            : result.failure
              ? volume.probe_failures.map(failure =>
                failure.path === result.path ? result.failure! : failure
              )
              : volume.probe_failures;
          return { ...volume, files, probe_failures: probeFailures };
        }),
      }));
    } catch (e) {
      set({ error: String(e) });
    } finally {
      set(state => {
        const retryingProbeFailures = { ...state.retryingProbeFailures };
        delete retryingProbeFailures[retryKey];
        return { retryingProbeFailures };
      });
    }
  },

  retryAllProbeFailures: async (volumeId) => {
    const volume = get().volumes.find(item => item.id === volumeId);
    if (!volume || volume.probe_failures.length === 0) return;
    if (Object.keys(get().retryingProbeFailures).some(key => key.startsWith(`${volumeId}:`))) return;

    const paths = volume.probe_failures.map(failure => failure.path);
    const retryKeys = paths.map(path => `${volumeId}:${path}`);
    set(state => ({
      retryingProbeBatches: { ...state.retryingProbeBatches, [volumeId]: true },
      retryingProbeFailures: {
        ...state.retryingProbeFailures,
        ...Object.fromEntries(retryKeys.map(key => [key, true])),
      },
    }));

    try {
      const results = await invoke<Array<{
        volume_id: string;
        path: string;
        file: Volume['files'][number] | null;
        failure: Volume['probe_failures'][number] | null;
      }>>('retry_all_probe_failures', { volumeId });

      set(state => ({
        volumes: state.volumes.map(currentVolume => {
          if (currentVolume.id !== volumeId) return currentVolume;
          let files = currentVolume.files;
          let probeFailures = currentVolume.probe_failures;

          for (const result of results) {
            if (result.file && !files.some(file => file.path === result.path)) {
              files = [...files, result.file];
            }
            if (result.file) {
              probeFailures = probeFailures.filter(failure => failure.path !== result.path);
            } else if (result.failure) {
              probeFailures = probeFailures.map(failure =>
                failure.path === result.path ? result.failure! : failure
              );
            }
          }

          return { ...currentVolume, files, probe_failures: probeFailures };
        }),
      }));
    } catch (e) {
      let error = String(e);
      try {
        const currentVolumes = await invoke<Volume[]>('get_current_volumes');
        set({ volumes: currentVolumes });
      } catch (refreshError) {
        error += `; also failed to refresh source files: ${String(refreshError)}`;
      }
      set({ error });
    } finally {
      set(state => {
        const retryingProbeFailures = { ...state.retryingProbeFailures };
        for (const key of retryKeys) delete retryingProbeFailures[key];
        const retryingProbeBatches = { ...state.retryingProbeBatches };
        delete retryingProbeBatches[volumeId];
        return { retryingProbeFailures, retryingProbeBatches };
      });
    }
  },

  clearVolumes: async () => {
    try {
      await invoke('clear_volumes');
      set({ volumes: [], selectedFileId: null, expandedVolumes: {} });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  selectFile: (fileId) => set({ selectedFileId: fileId }),

  toggleVolumeExpanded: (volumeId) => {
    set(state => ({
      expandedVolumes: {
        ...state.expandedVolumes,
        [volumeId]: !state.expandedVolumes[volumeId],
      },
    }));
  },

  updateFileCategory: async (volumeId, fileId, category) => {
    try {
      await invoke('update_file_category', { volumeId, fileId, category });
      set(state => ({
        volumes: state.volumes.map(v =>
          v.id !== volumeId ? v : {
            ...v,
            files: v.files.map(f =>
              f.id !== fileId ? f : { ...f, category }
            ),
          }
        ),
      }));
    } catch (e) {
      set({ error: String(e) });
    }
  },

  updateTrack: async (volumeId, fileId, trackId, language, name) => {
    try {
      await invoke('update_track', { volumeId, fileId, trackId, language, name });
      set(state => ({
        volumes: state.volumes.map(v =>
          v.id !== volumeId ? v : {
            ...v,
            files: v.files.map(f =>
              f.id !== fileId ? f : {
                ...f,
                tracks: f.tracks.map(t =>
                  t.id !== trackId ? t : { ...t, language, name }
                ),
              }
            ),
          }
        ),
      }));
    } catch (e) {
      set({ error: String(e) });
    }
  },

  createProject: async () => {
    const { outputName, outputPath } = get();
    if (!outputName || !outputPath) {
      set({ error: 'Set an output name and output path before adding to queue.' });
      return;
    }
    try {
      const project = await invoke<Project>('create_project', {
        outputName,
        outputPath,
      });
      set(state => ({ queue: [...state.queue, project] }));
    } catch (e) {
      set({ error: String(e) });
    }
  },

  removeProject: async (projectId) => {
    try {
      await invoke('remove_project', { projectId });
      set(state => ({ queue: state.queue.filter(p => p.id !== projectId) }));
    } catch (e) {
      set({ error: String(e) });
    }
  },

  changeProjectOutputPath: async (projectId) => {
    const project = get().queue.find(item => item.id === projectId);
    if (!project || project.status !== 'error') return;

    try {
      const selected = await open({
        directory: true,
        multiple: false,
        defaultPath: project.output_path,
        title: 'Select output folder',
      });
      if (!selected || Array.isArray(selected)) return;

      const updatedProject = await invoke<Project>('update_project_output_path', {
        projectId,
        outputPath: selected,
      });
      set(state => ({
        error: null,
        queue: state.queue.map(item => item.id === updatedProject.id ? updatedProject : item),
      }));
    } catch (e) {
      set({ error: String(e) });
    }
  },

  retryProject: async (projectId) => {
    try {
      await retryProjectWithQueue(projectId, {
        retry: (id) => invoke<Project>('retry_project', { projectId: id }),
        startMuxing: () => get().startMuxing(),
        getMuxerStatus: () => get().muxerStatus,
        onRetry: (project) => set(state => ({
          error: null,
          queue: state.queue.map(item => item.id === project.id ? project : item),
        })),
        onRetrying: (id, isRetrying) => set(state => {
          const retryingProjects = { ...state.retryingProjects };
          if (isRetrying) retryingProjects[id] = true;
          else delete retryingProjects[id];
          return { retryingProjects };
        }),
      });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  startMuxing: async () => {
    try {
      await invoke('start_muxing');
      set({ muxerStatus: { is_running: true, is_paused: false } });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  pauseMuxing: async () => {
    try {
      await invoke('pause_muxing');
      set({ muxerStatus: { is_running: true, is_paused: true } });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  resumeMuxing: async () => {
    try {
      await invoke('resume_muxing');
      set({ muxerStatus: { is_running: true, is_paused: false } });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  stopMuxing: async () => {
    try {
      await invoke('stop_muxing');
      set({ muxerStatus: { is_running: false, is_paused: false } });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  loadSettings: async () => {
    try {
      const response = await invoke<{
        settings: Settings;
        recovery_warning: string | null;
        recovery_backup_available: boolean;
        settings_save_allowed: boolean;
        recovery_retry_available: boolean;
      }>('get_settings');
      set(getSettingsLoadUpdate(response));
    } catch (e) {
      set({ error: String(e) });
    }
  },

  retryRecoveryCopy: async () => {
    set({ isRetryingRecoveryCopy: true, error: null });
    try {
      const response = await invoke<{
        settings: Settings;
        recovery_warning: string | null;
        recovery_backup_available: boolean;
        settings_save_allowed: boolean;
        recovery_retry_available: boolean;
      }>('retry_settings_recovery_copy');
      set({
        ...getSettingsLoadUpdate(response),
        isRetryingRecoveryCopy: false,
        error: null,
      });
      return true;
    } catch (e) {
      set({ isRetryingRecoveryCopy: false, error: String(e) });
      return false;
    }
  },

  openRecoveryBackup: async () => {
    set({ recoveryBackupOpenError: null });
    try {
      await invoke('open_settings_recovery_backup');
    } catch (e) {
      set({ recoveryBackupOpenError: String(e) });
    }
  },

  previewRecoveryBackup: async () => {
    set({
      recoveryRestorePreview: null,
      recoveryRestoreError: null,
      isPreparingRecoveryRestore: true,
    });
    try {
      const recoveryRestorePreview = await invoke<SettingsRecoveryPreview>(
        'preview_settings_recovery_backup',
      );
      set({ recoveryRestorePreview, isPreparingRecoveryRestore: false });
    } catch (e) {
      set({
        recoveryRestoreError: String(e),
        isPreparingRecoveryRestore: false,
      });
    }
  },

  restoreRecoveryBackup: async () => {
    const preview = get().recoveryRestorePreview;
    if (!preview) return;

    set({ isRestoringRecoveryBackup: true, recoveryRestoreError: null });
    try {
      const response = await invoke<{
        settings: Settings;
        recovery_warning: string | null;
      }>('restore_settings_recovery_backup', {
        expectedSourceHash: preview.source_hash,
      });
      set({
        ...getSettingsSaveSuccessUpdate(response.settings, response.recovery_warning),
        recoveryRestorePreview: null,
        recoveryRestoreError: null,
        isRestoringRecoveryBackup: false,
      });
    } catch (e) {
      set({
        recoveryRestoreError: String(e),
        isRestoringRecoveryBackup: false,
      });
    }
  },

  dismissRecoveryRestorePreview: () => set({
    recoveryRestorePreview: null,
    recoveryRestoreError: null,
    isPreparingRecoveryRestore: false,
  }),

  dismissRecoveryWarning: () => set(getRecoveryWarningDismissUpdate()),
  dismissRecoveryBackupOpenError: () => set({ recoveryBackupOpenError: null }),

  saveSettings: async (settings) => {
    try {
      const response = await invoke<{
        settings: Settings;
        recovery_warning: string | null;
      }>('save_settings', { settings });
      set(getSettingsSaveSuccessUpdate(response.settings, response.recovery_warning));
      return true;
    } catch (e) {
      set(getSettingsSaveFailureUpdate(e));
      return false;
    }
  },

  loadQueue: async () => {
    try {
      const queue = await invoke<Project[]>('get_queue');
      set({ queue });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  loadCurrentVolumes: async () => {
    try {
      const volumes = await invoke<Volume[]>('get_current_volumes');
      const expandedVolumes: Record<string, boolean> = {};
      volumes.forEach(volume => { expandedVolumes[volume.id] = true; });
      set({ volumes, expandedVolumes });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  loadTrackTemplates: async () => {
    try {
      const trackTemplates = await invoke<Record<string, Track[]>>('get_track_templates');
      set({ trackTemplates });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  saveTrackTemplate: async (category, template) => {
    try {
      await invoke('save_track_template', { category, template });
      set(state => ({
        trackTemplates: { ...state.trackTemplates, [category]: template },
      }));
    } catch (e) {
      set({ error: String(e) });
    }
  },

  setOutputName: (outputName) => set({ outputName }),

  selectOutputPath: async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select output folder',
      });
      if (selected && !Array.isArray(selected)) {
        set({ outputPath: selected });
      }
    } catch (e) {
      set({ error: String(e) });
    }
  },

  setupEventListeners: () => {
    listen<MuxProgress>('mux-progress', ({ payload }) => {
      set(state => ({
        queue: state.queue.map(p =>
          p.id !== payload.project_id ? p : {
            ...p,
            status: payload.status as Project['status'],
            progress: payload.progress,
            current_file: payload.current_file,
            resume_reason: payload.resume_reason ?? null,
            error: payload.error || undefined,
          }
        ),
      }));
    });

    listen('mux-complete', () => {
      set({ muxerStatus: { is_running: false, is_paused: false } });
    });

    listen<string>('app-error', ({ payload }) => {
      set({ error: payload });
    });

    listen<{ is_running: boolean; persistence_retry_required: boolean }>(
      'mux-close-requested',
      async ({ payload }) => {
        await handleMuxCloseRequested(
          () => {
            const prompt = getMuxCloseConfirmation(
              payload.is_running,
              payload.persistence_retry_required,
            );
            return confirm(prompt.message, { title: prompt.title, type: 'warning' });
          },
          () => invoke('confirm_close'),
          (error) => set({ error }),
        );
      },
    );
  },
}));
