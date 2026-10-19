import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/tauri';
import { confirm, open } from '@tauri-apps/api/dialog';
import { listen } from '@tauri-apps/api/event';
import type { Volume, Project, Settings, Track, FileCategory } from './types';
import { handleMuxCloseRequested } from './muxCloseConfirmation.js';
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
  trackTemplates: Record<string, Track[]>;
  selectedFileId: string | null;
  expandedVolumes: Record<string, boolean>;
  muxerStatus: MuxerStatus;
  isLoading: boolean;
  error: string | null;
  outputName: string;
  outputPath: string;

  // Actions
  loadBdmv: () => Promise<void>;
  loadMultipleBdmvs: () => Promise<void>;
  clearVolumes: () => Promise<void>;
  selectFile: (fileId: string) => void;
  toggleVolumeExpanded: (volumeId: string) => void;
  updateFileCategory: (volumeId: string, fileId: string, category: FileCategory) => Promise<void>;
  updateTrack: (volumeId: string, fileId: string, trackId: number, language: string, name: string) => Promise<void>;
  createProject: () => Promise<void>;
  removeProject: (projectId: string) => Promise<void>;
  retryProject: (projectId: string) => Promise<void>;
  startMuxing: () => Promise<void>;
  pauseMuxing: () => Promise<void>;
  resumeMuxing: () => Promise<void>;
  stopMuxing: () => Promise<void>;
  loadSettings: () => Promise<void>;
  openRecoveryBackup: () => Promise<void>;
  dismissRecoveryWarning: () => void;
  saveSettings: (settings: Settings) => Promise<void>;
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
  trackTemplates: {},
  selectedFileId: null,
  expandedVolumes: {},
  muxerStatus: { is_running: false, is_paused: false },
  isLoading: false,
  error: null,
  outputName: '',
  outputPath: '',

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

  retryProject: async (projectId) => {
    try {
      const project = await invoke<Project>('retry_project', { projectId });
      set(state => ({
        error: null,
        queue: state.queue.map(item => item.id === project.id ? project : item),
      }));

      if (!get().muxerStatus.is_running) {
        await get().startMuxing();
      }
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
      }>('get_settings');
      set(getSettingsLoadUpdate(response));
    } catch (e) {
      set({ error: String(e) });
    }
  },

  openRecoveryBackup: async () => {
    try {
      await invoke('open_settings_recovery_backup');
    } catch (e) {
      set({ error: String(e) });
    }
  },

  dismissRecoveryWarning: () => set(getRecoveryWarningDismissUpdate()),

  saveSettings: async (settings) => {
    try {
      await invoke('save_settings', { settings });
      set(getSettingsSaveSuccessUpdate(settings));
    } catch (e) {
      set(getSettingsSaveFailureUpdate(e));
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

    listen('mux-close-requested', async () => {
      await handleMuxCloseRequested(
        () => confirm(
          'Muxing is still in progress. Close the app? Incomplete projects will be saved as Paused.',
          { title: 'Muxing in progress', type: 'warning' },
        ),
        () => invoke('confirm_close'),
        (error) => set({ error }),
      );
    });
  },
}));
