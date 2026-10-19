import React, { useEffect, useState } from 'react';
import { useAppStore } from './store';
import { confirm, open } from '@tauri-apps/api/dialog';
import { 
  FolderOpen, Play, Pause, Settings, ChevronRight, ChevronDown, 
  Film, Tv, Music, Star, AlertTriangle, Check, X, Edit3, 
  Clock, HardDrive, Layers, Plus, Trash2, Square,
  FileVideo, List, Save, FolderInput, Grip, RotateCw
} from 'lucide-react';
import { formatDuration, formatSize, getCategoryColor, getFileNumber, formatFileLabel } from './utils/format';
import type { MediaFile, Track, Volume, Project, Settings as SettingsType, FileCategory } from './types';
import { CATEGORIES, LANGUAGES } from './types';
import {
  getSettingsSaveState,
  getVisibleNotice,
  isRecoveryBackupVisible,
} from './settingsRecoveryNotice.js';
import './App.css';

// ============================================================================
// File Item Component
// ============================================================================

interface FileItemProps {
  file: MediaFile;
  volumeId: string;
  isSelected: boolean;
  onSelect: () => void;
  onEdit: () => void;
}

const FileItem: React.FC<FileItemProps> = ({ file, volumeId, isSelected, onSelect, onEdit }) => {
  const { updateFileCategory } = useAppStore();
  const category = CATEGORIES[file.category];
  const color = getCategoryColor(file.category);
  
  const handleCategoryChange = async (e: React.ChangeEvent<HTMLSelectElement>) => {
    e.stopPropagation();
    await updateFileCategory(volumeId, file.id, e.target.value as FileCategory);
  };
  
  const CategoryIcon = file.category === 'EPISODE' ? Tv 
    : file.category === 'OP' || file.category === 'ED' ? Music 
    : file.category === 'SPECIAL' ? Star 
    : FileVideo;
  
  return (
    <div 
      className={`file-item ${isSelected ? 'selected' : ''}`}
      onClick={onSelect}
      style={{ borderLeftColor: color }}
    >
      <div className="file-item-grip">
        <Grip size={14} />
      </div>
      <div className="file-item-icon" style={{ color }}>
        <CategoryIcon size={18} />
      </div>
      <div className="file-item-info">
        <div className="file-item-name">{file.filename}</div>
        <div className="file-item-meta">
          <span><Clock size={12} /> {formatDuration(file.duration)}</span>
          <span><HardDrive size={12} /> {formatSize(file.size)}</span>
          <span><Layers size={12} /> {file.tracks?.length || 0} tracks</span>
        </div>
      </div>
      <div className="file-item-category">
        <select 
          value={file.category}
          onChange={handleCategoryChange}
          onClick={(e) => e.stopPropagation()}
          style={{ borderColor: color }}
        >
          {Object.values(CATEGORIES).map(cat => (
            <option key={cat.id} value={cat.id}>{cat.label}</option>
          ))}
        </select>
      </div>
      <div className="file-item-number">
        {formatFileLabel(file.category, getFileNumber(file))}
      </div>
      <button className="file-item-edit" onClick={(e) => { e.stopPropagation(); onEdit(); }}>
        <Edit3 size={14} />
      </button>
    </div>
  );
};

// ============================================================================
// Volume Card Component
// ============================================================================

interface VolumeCardProps {
  volume: Volume;
  isExpanded: boolean;
  onToggle: () => void;
  selectedFileId: string | null;
  onFileSelect: (fileId: string) => void;
  onEditFile: (file: MediaFile) => void;
}

const VolumeCard: React.FC<VolumeCardProps> = ({ 
  volume, isExpanded, onToggle, selectedFileId, onFileSelect, onEditFile 
}) => {
  const categoryCounts = volume.files.reduce((acc, f) => {
    acc[f.category] = (acc[f.category] || 0) + 1;
    return acc;
  }, {} as Record<string, number>);
  
  return (
    <div className={`volume-card ${isExpanded ? 'expanded' : ''}`}>
      <div className="volume-header" onClick={onToggle}>
        <div className="volume-expand">
          {isExpanded ? <ChevronDown size={20} /> : <ChevronRight size={20} />}
        </div>
        <div className="volume-title">
          <Film size={18} />
          <span>{volume.name}</span>
          <span className="volume-number">Vol. {volume.volume_number}</span>
        </div>
        <div className="volume-stats">
          {Object.entries(categoryCounts).map(([cat, count]) => (
            <span 
              key={cat} 
              className="category-badge" 
              style={{ 
                backgroundColor: getCategoryColor(cat) + '22', 
                color: getCategoryColor(cat) 
              }}
            >
              {count} {CATEGORIES[cat as FileCategory]?.label.split(' ')[0]}
            </span>
          ))}
        </div>
        <div className="volume-total">
          {volume.files.length} files
        </div>
      </div>
      
      {isExpanded && (
        <div className="volume-files">
          {volume.probe_failures?.length > 0 && (
            <section className="probe-failures" aria-label="FFprobe failures">
              <h4>
                <AlertTriangle size={16} />
                {volume.probe_failures.length} source {volume.probe_failures.length === 1 ? 'file was' : 'files were'} skipped
              </h4>
              <ul>
                {volume.probe_failures.map((failure, index) => (
                  <li key={`${failure.path}-${index}`}>
                    <strong>{failure.path}</strong>
                    <pre>{failure.diagnostic}</pre>
                  </li>
                ))}
              </ul>
            </section>
          )}
          {volume.files.map(file => (
            <FileItem 
              key={file.id}
              file={file}
              volumeId={volume.id}
              isSelected={selectedFileId === file.id}
              onSelect={() => onFileSelect(file.id)}
              onEdit={() => onEditFile(file)}
            />
          ))}
        </div>
      )}
    </div>
  );
};

// ============================================================================
// Queue Item Component
// ============================================================================

interface QueueItemProps {
  project: Project;
  isActive: boolean;
  onRemove: () => void;
  onRetry: () => void;
  onChangeOutputPath: () => void;
}

const QueueItem: React.FC<QueueItemProps> = ({
  project,
  isActive,
  onRemove,
  onRetry,
  onChangeOutputPath,
}) => {
  const statusColors: Record<string, string> = {
    pending: '#94a3b8',
    processing: '#4ade80',
    paused: '#fbbf24',
    complete: '#22c55e',
    error: '#ef4444'
  };
  
  return (
    <div className={`queue-item ${isActive ? 'active' : ''}`}>
      <div className="queue-status" style={{ backgroundColor: statusColors[project.status] }} />
      <div className="queue-info">
        <div className="queue-name">{project.output_name}</div>
        <div className="queue-progress">
          {project.status === 'processing' && (
            <>
              <div className="progress-bar">
                <div className="progress-fill" style={{ width: `${project.progress}%` }} />
              </div>
              <span>{Math.round(project.progress)}%</span>
            </>
          )}
          {project.status === 'pending' && <span>Waiting...</span>}
          {project.status === 'paused' && <span>Paused</span>}
          {project.status === 'complete' && <span>Complete</span>}
          {project.status === 'error' && <span className="error-text">Error</span>}
        </div>
        {project.current_file && project.status === 'processing' && (
          <div className="queue-current-file">{project.current_file}</div>
        )}
        {project.current_file && project.resume_reason && project.status === 'processing' && (
          <div className="queue-resume-reason">{project.resume_reason}</div>
        )}
        {project.error && (
          <div className="queue-error-detail" title={project.error}>{project.error}</div>
        )}
        {project.status === 'error' && (
          <div className="queue-output-path" title={project.output_path}>
            Output: {project.output_path}
          </div>
        )}
      </div>
      {project.status === 'error' && (
        <div className="queue-error-actions">
          <button
            className="queue-change-output"
            onClick={onChangeOutputPath}
            title="Change output folder before retrying"
            aria-label={`Change output folder for ${project.output_name}`}
          >
            <FolderInput size={14} />
          </button>
          <button className="queue-retry" onClick={onRetry} title="Retry unfinished files">
            <RotateCw size={14} /> Retry
          </button>
        </div>
      )}
      <button className="queue-remove" onClick={onRemove}>
        <X size={14} />
      </button>
    </div>
  );
};

// ============================================================================
// Track Editor Modal
// ============================================================================

interface TrackEditorProps {
  file: MediaFile;
  volumeId: string;
  onClose: () => void;
}

const TrackEditor: React.FC<TrackEditorProps> = ({ file, volumeId, onClose }) => {
  const { updateTrack } = useAppStore();
  const [localTracks, setLocalTracks] = useState<Track[]>(file.tracks);
  
  const handleTrackChange = (trackId: number, field: 'language' | 'name', value: string) => {
    setLocalTracks(tracks => 
      tracks.map(t => t.id === trackId ? { ...t, [field]: value } : t)
    );
  };
  
  const handleSave = async () => {
    for (const track of localTracks) {
      const original = file.tracks.find(t => t.id === track.id);
      if (original && (original.language !== track.language || original.name !== track.name)) {
        await updateTrack(volumeId, file.id, track.id, track.language, track.name);
      }
    }
    onClose();
  };
  
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal track-editor" onClick={e => e.stopPropagation()}>
        <div className="modal-header">
          <h3><Layers size={20} /> Track Editor - {file.filename}</h3>
          <button className="modal-close" onClick={onClose}><X size={20} /></button>
        </div>
        <div className="modal-body">
          <div className="file-preview">
            <div className="preview-stat">
              <span>Duration:</span> {formatDuration(file.duration)}
            </div>
            <div className="preview-stat">
              <span>Size:</span> {formatSize(file.size)}
            </div>
            <div className="preview-stat">
              <span>Category:</span> {CATEGORIES[file.category].label}
            </div>
          </div>
          
          <table className="track-table">
            <thead>
              <tr>
                <th>ID</th>
                <th>Type</th>
                <th>Codec</th>
                <th>Details</th>
                <th>Language</th>
                <th>Track Name</th>
              </tr>
            </thead>
            <tbody>
              {localTracks.map(track => (
                <tr key={track.id}>
                  <td>{track.id}</td>
                  <td>
                    <span className={`track-type ${track.track_type}`}>
                      {track.track_type}
                    </span>
                  </td>
                  <td>{track.codec}</td>
                  <td>{track.resolution || track.channels || '-'}</td>
                  <td>
                    <select 
                      value={track.language}
                      onChange={e => handleTrackChange(track.id, 'language', e.target.value)}
                    >
                      {LANGUAGES.map(lang => (
                        <option key={lang.code} value={lang.code}>{lang.name}</option>
                      ))}
                    </select>
                  </td>
                  <td>
                    <input 
                      type="text"
                      value={track.name}
                      onChange={e => handleTrackChange(track.id, 'name', e.target.value)}
                      placeholder="Track name..."
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="modal-footer">
          <button className="btn secondary" onClick={onClose}>Cancel</button>
          <button className="btn primary" onClick={handleSave}>
            <Save size={16} /> Save Tracks
          </button>
        </div>
      </div>
    </div>
  );
};

// ============================================================================
// Settings Modal
// ============================================================================

interface SettingsModalProps {
  settings: SettingsType;
  recoveryBackupAvailable: boolean;
  settingsSaveAllowed: boolean;
  recoveryWarning: string | null;
  onRestoreBackup: () => void;
  onClose: () => void;
}

const SettingsModal: React.FC<SettingsModalProps> = ({
  settings: initialSettings,
  recoveryBackupAvailable,
  settingsSaveAllowed,
  recoveryWarning,
  onRestoreBackup,
  onClose,
}) => {
  const { saveSettings } = useAppStore();
  const [settings, setSettings] = useState<SettingsType>(initialSettings);
  const [browseError, setBrowseError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const saveState = getSettingsSaveState(settingsSaveAllowed, recoveryWarning);
  
  const handleSave = async () => {
    setSaveError(null);
    const saved = await saveSettings(settings);
    if (saved) {
      onClose();
    } else {
      setSaveError(useAppStore.getState().error || 'Settings could not be saved.');
    }
  };

  const browseForExecutable = async (setting: 'mkvmerge_path' | 'ffprobe_path', title: string) => {
    try {
      const selected = await open({ title, multiple: false, directory: false });
      if (typeof selected === 'string') {
        setSettings(current => ({ ...current, [setting]: selected }));
      }
      setBrowseError(null);
    } catch (error) {
      setBrowseError(`Could not open the file picker: ${String(error)}`);
    }
  };
  
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal settings-modal" onClick={e => e.stopPropagation()}>
        <div className="modal-header">
          <h3><Settings size={20} /> Settings</h3>
          <button className="modal-close" onClick={onClose}><X size={20} /></button>
        </div>
        <div className="modal-body settings-panel">
          {(saveState.warning || recoveryWarning) && (
            <p className="settings-error" role={saveState.warning ? 'alert' : 'status'}>
              {saveState.warning || recoveryWarning}
            </p>
          )}
          {saveError && (
            <p className="settings-error" role="alert">{saveError}</p>
          )}
          <div className="settings-group">
            <h4>Tools</h4>
            
            <div className="setting-row">
              <div className="setting-label">
                MKVToolNix Path
                <small>mkvmerge executable name or full path</small>
              </div>
              <div className="setting-input">
                <input 
                  type="text"
                  value={settings.mkvmerge_path}
                  onChange={e => setSettings({ ...settings, mkvmerge_path: e.target.value })}
                  style={{ width: '220px' }}
                />
                <button
                  className="btn secondary"
                  onClick={() => browseForExecutable('mkvmerge_path', 'Select mkvmerge executable')}
                  aria-label="Browse for mkvmerge executable"
                >
                  <FolderOpen size={15} /> Browse
                </button>
              </div>
            </div>
            
            <div className="setting-row">
              <div className="setting-label">
                FFprobe Path
                <small>ffprobe executable name or full path</small>
              </div>
              <div className="setting-input">
                <input 
                  type="text"
                  value={settings.ffprobe_path}
                  onChange={e => setSettings({ ...settings, ffprobe_path: e.target.value })}
                  style={{ width: '220px' }}
                />
                <button
                  className="btn secondary"
                  onClick={() => browseForExecutable('ffprobe_path', 'Select ffprobe executable')}
                  aria-label="Browse for ffprobe executable"
                >
                  <FolderOpen size={15} /> Browse
                </button>
              </div>
            </div>
            {browseError && <p className="settings-error" role="alert">{browseError}</p>}
          </div>
          
          <div className="settings-group">
            <h4>Detection Thresholds</h4>
            
            <div className="setting-row">
              <div className="setting-label">
                Minimum Episode Duration
                <small>Files longer than this are classified as episodes</small>
              </div>
              <div className="setting-input">
                <input 
                  type="number"
                  value={settings.min_episode_duration / 60}
                  onChange={e => setSettings({ ...settings, min_episode_duration: Number(e.target.value) * 60 })}
                />
                <span>min</span>
              </div>
            </div>
            
            <div className="setting-row">
              <div className="setting-label">
                Minimum Extra Duration
                <small>Files shorter than episodes but longer than this are extras</small>
              </div>
              <div className="setting-input">
                <input 
                  type="number"
                  value={settings.min_extra_duration / 60}
                  onChange={e => setSettings({ ...settings, min_extra_duration: Number(e.target.value) * 60 })}
                />
                <span>min</span>
              </div>
            </div>
            
            <div className="setting-row">
              <div className="setting-label">
                OP/ED Duration Range
                <small>Files within this range are potential openings/endings</small>
              </div>
              <div className="setting-input">
                <input 
                  type="number"
                  value={settings.op_ed_min_duration}
                  onChange={e => setSettings({ ...settings, op_ed_min_duration: Number(e.target.value) })}
                  style={{ width: '70px' }}
                />
                <span>-</span>
                <input 
                  type="number"
                  value={settings.op_ed_max_duration}
                  onChange={e => setSettings({ ...settings, op_ed_max_duration: Number(e.target.value) })}
                  style={{ width: '70px' }}
                />
                <span>sec</span>
              </div>
            </div>
          </div>
          
          <div className="settings-group">
            <h4>Output Folders</h4>
            
            <div className="setting-row">
              <div className="setting-label">
                Episode Folder Name
              </div>
              <div className="setting-input">
                <input 
                  type="text"
                  value={settings.episode_folder_name}
                  onChange={e => setSettings({ ...settings, episode_folder_name: e.target.value })}
                  style={{ width: '150px' }}
                />
              </div>
            </div>
            
            <div className="setting-row">
              <div className="setting-label">
                Extra Folder Name
              </div>
              <div className="setting-input">
                <input 
                  type="text"
                  value={settings.extra_folder_name}
                  onChange={e => setSettings({ ...settings, extra_folder_name: e.target.value })}
                  style={{ width: '150px' }}
                />
              </div>
            </div>
          </div>

          {recoveryBackupAvailable && (
            <div className="settings-group">
              <h4>Preference Recovery</h4>
              <p className="recovery-settings-copy">
                Review and restore preferences from the preserved backup. The backup is kept unchanged.
              </p>
              <div>
                <button className="btn secondary" onClick={onRestoreBackup}>
                  <RotateCw size={15} /> Review backup for restore
                </button>
              </div>
            </div>
          )}
        </div>
        <div className="modal-footer">
          <button className="btn secondary" onClick={onClose}>Cancel</button>
          <button className="btn primary" onClick={handleSave} disabled={saveState.disabled}>
            <Check size={16} /> Save Settings
          </button>
        </div>
      </div>
    </div>
  );
};

// ============================================================================
// Template Editor Modal
// ============================================================================

interface TemplateEditorProps {
  category: FileCategory;
  template: Track[] | undefined;
  onClose: () => void;
}

const TemplateEditor: React.FC<TemplateEditorProps> = ({ category, template, onClose }) => {
  const { saveTrackTemplate } = useAppStore();
  const [tracks, setTracks] = useState<Track[]>(template || [
    { id: 0, track_type: 'video', codec: '', language: 'und', name: '', default: true, forced: false },
    { id: 1, track_type: 'audio', codec: '', language: 'jpn', name: 'Japanese', default: true, forced: false },
  ]);
  
  const addTrack = () => {
    const newId = Math.max(...tracks.map(t => t.id), -1) + 1;
    setTracks([...tracks, { 
      id: newId, 
      track_type: 'audio', 
      codec: '', 
      language: 'und', 
      name: '', 
      default: false, 
      forced: false 
    }]);
  };
  
  const removeTrack = (id: number) => {
    setTracks(tracks.filter(t => t.id !== id));
  };
  
  const updateTrack = (id: number, field: keyof Track, value: any) => {
    setTracks(tracks.map(t => t.id === id ? { ...t, [field]: value } : t));
  };
  
  const handleSave = async () => {
    await saveTrackTemplate(category, tracks);
    onClose();
  };
  
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal layout-editor" onClick={e => e.stopPropagation()}>
        <div className="modal-header">
          <h3>
            <Settings size={20} /> 
            Track Layout Template - {CATEGORIES[category].label}
          </h3>
          <button className="modal-close" onClick={onClose}><X size={20} /></button>
        </div>
        <div className="modal-body">
          <p className="template-hint">
            Define the expected track layout for all {CATEGORIES[category].label.toLowerCase()}. 
            Files with different layouts will be flagged for review.
          </p>
          
          <div className="template-tracks">
            {tracks.map((track, index) => (
              <div key={track.id} className="template-track">
                <span className="track-order">{index + 1}</span>
                <select 
                  value={track.track_type}
                  onChange={e => updateTrack(track.id, 'track_type', e.target.value)}
                >
                  <option value="video">Video</option>
                  <option value="audio">Audio</option>
                  <option value="subtitle">Subtitle</option>
                </select>
                <select 
                  value={track.language}
                  onChange={e => updateTrack(track.id, 'language', e.target.value)}
                >
                  {LANGUAGES.map(lang => (
                    <option key={lang.code} value={lang.code}>{lang.name}</option>
                  ))}
                </select>
                <input 
                  type="text"
                  value={track.name}
                  onChange={e => updateTrack(track.id, 'name', e.target.value)}
                  placeholder="Track name..."
                />
                <button className="btn icon danger" onClick={() => removeTrack(track.id)}>
                  <Trash2 size={14} />
                </button>
              </div>
            ))}
          </div>
          
          <button className="btn secondary add-track" onClick={addTrack}>
            <Plus size={16} /> Add Track
          </button>
        </div>
        <div className="modal-footer">
          <button className="btn secondary" onClick={onClose}>Cancel</button>
          <button className="btn primary" onClick={handleSave}>
            <Save size={16} /> Save Template
          </button>
        </div>
      </div>
    </div>
  );
};

// ============================================================================
// Main App Component
// ============================================================================

const App: React.FC = () => {
  const {
    volumes,
    queue,
    settings,
    trackTemplates,
    selectedFileId,
    expandedVolumes,
    muxerStatus,
    isLoading,
    error,
    recoveryWarning,
    recoveryBackupAvailable,
    settingsSaveAllowed,
    recoveryBackupOpenError,
    recoveryRestorePreview,
    recoveryRestoreError,
    isPreparingRecoveryRestore,
    isRestoringRecoveryBackup,
    outputName,
    outputPath,
    loadBdmv,
    loadMultipleBdmvs,
    clearVolumes,
    selectFile,
    toggleVolumeExpanded,
    createProject,
    removeProject,
    changeProjectOutputPath,
    retryProject,
    startMuxing,
    pauseMuxing,
    resumeMuxing,
    stopMuxing,
    loadSettings,
    openRecoveryBackup,
    previewRecoveryBackup,
    restoreRecoveryBackup,
    dismissRecoveryRestorePreview,
    dismissRecoveryWarning,
    dismissRecoveryBackupOpenError,
    loadQueue,
    loadCurrentVolumes,
    loadTrackTemplates,
    setOutputName,
    selectOutputPath,
    setupEventListeners,
  } = useAppStore();
  
  const [showSettings, setShowSettings] = useState(false);
  const [editingFile, setEditingFile] = useState<{ file: MediaFile; volumeId: string } | null>(null);
  const [editingTemplate, setEditingTemplate] = useState<FileCategory | null>(null);
  const visibleNotice = getVisibleNotice(recoveryWarning, error, recoveryBackupOpenError);
  const showRecoveryBackup = isRecoveryBackupVisible(recoveryWarning, recoveryBackupAvailable);

  const handleRestoreRecoveryBackup = async () => {
    if (!recoveryRestorePreview) return;
    try {
      const confirmed = await confirm(
        'Replace the current preferences with the reviewed backup settings? The preserved backup will not be changed.',
        { title: 'Confirm preferences restore', type: 'warning' },
      );
      if (confirmed) await restoreRecoveryBackup();
    } catch (restoreError) {
      useAppStore.setState({ recoveryRestoreError: String(restoreError) });
    }
  };
  
  // Initialize app
  useEffect(() => {
    loadSettings();
    loadQueue();
    loadCurrentVolumes();
    loadTrackTemplates();
    setupEventListeners();
  }, []);
  
  // Get volume ID for selected file
  const getVolumeIdForFile = (fileId: string): string | null => {
    for (const vol of volumes) {
      if (vol.files.some(f => f.id === fileId)) {
        return vol.id;
      }
    }
    return null;
  };
  
  const handleToggleMuxing = () => {
    if (muxerStatus.is_running) {
      if (muxerStatus.is_paused) {
        resumeMuxing();
      } else {
        pauseMuxing();
      }
    } else {
      startMuxing();
    }
  };
  
  return (
    <div className="app">
      {/* Header */}
      <header className="header">
        <h1 className="logo" aria-label="BDMV Remuxer main interface">
          <div className="logo-icon">
            <Film size={24} color="white" />
          </div>
          BDMV Remuxer
        </h1>
        <div className="header-actions">
          <button className="btn secondary" onClick={() => setShowSettings(true)}>
            <Settings size={18} /> Settings
          </button>
        </div>
      </header>
      
      {/* Error Toast */}
      {visibleNotice && (
        <div className="error-toast">
          <AlertTriangle size={18} />
          <span>{visibleNotice}</span>
          {showRecoveryBackup && (
            <>
              <button
                className="recovery-backup-button"
                onClick={openRecoveryBackup}
                title="Open the preserved preferences backup"
              >
                <FolderOpen size={15} /> Open backup
              </button>
              <button
                className="recovery-backup-button"
                onClick={previewRecoveryBackup}
                title="Review and restore preferences from the preserved backup"
              >
                <RotateCw size={15} /> Restore backup
              </button>
            </>
          )}
          <button
            onClick={() => {
              if (recoveryWarning) dismissRecoveryWarning();
              else useAppStore.setState({ error: null });
              dismissRecoveryBackupOpenError();
            }}
          >
            <X size={16} />
          </button>
        </div>
      )}
      
      {/* Main Layout */}
      <main className="main">
        {/* Sidebar - Queue */}
        <aside className="sidebar">
          <div className="sidebar-header">
            <div className="sidebar-title">Muxing Queue</div>
          </div>
          <div className="queue-list">
            {queue.length === 0 ? (
              <div className="queue-empty">
                <List size={32} />
                <p>No projects in queue</p>
              </div>
            ) : (
              queue.map(project => (
                <QueueItem 
                  key={project.id}
                  project={project}
                  isActive={project.status === 'processing'}
                  onRemove={() => removeProject(project.id)}
                  onRetry={() => retryProject(project.id)}
                  onChangeOutputPath={() => changeProjectOutputPath(project.id)}
                />
              ))
            )}
          </div>
          <div className="sidebar-controls">
            {muxerStatus.is_running && (
              <button className="btn danger" onClick={stopMuxing}>
                <Square size={16} /> Stop
              </button>
            )}
            <button 
              className={`btn ${muxerStatus.is_running && !muxerStatus.is_paused ? 'secondary' : 'primary'}`}
              onClick={handleToggleMuxing}
              disabled={queue.length === 0 || queue.every(p => p.status === 'complete')}
            >
              {muxerStatus.is_running && !muxerStatus.is_paused 
                ? <><Pause size={16} /> Pause</> 
                : <><Play size={16} /> {muxerStatus.is_paused ? 'Resume' : 'Start'}</>
              }
            </button>
          </div>
        </aside>
        
        {/* Content */}
        <div className="content">
          {/* Toolbar */}
          <div className="toolbar">
            <div className="output-config">
              <div className="output-name">
                <label>Output Name:</label>
                <input 
                  type="text"
                  value={outputName}
                  onChange={(e) => setOutputName(e.target.value)}
                  placeholder="[Judas] Show Name"
                />
              </div>
              <div className="output-path">
                <label>Output Path:</label>
                <input 
                  type="text"
                  value={outputPath}
                  readOnly
                  placeholder="Select output folder..."
                />
                <button className="btn icon" onClick={selectOutputPath}>
                  <FolderInput size={18} />
                </button>
              </div>
            </div>
            <div className="toolbar-actions">
              <button className="btn secondary" onClick={loadBdmv} disabled={isLoading}>
                <FolderOpen size={18} /> Load Volume
              </button>
              <button className="btn secondary" onClick={loadMultipleBdmvs} disabled={isLoading}>
                <Plus size={18} /> Load Multiple
              </button>
              {volumes.length > 0 && (
                <button className="btn secondary" onClick={clearVolumes}>
                  <Trash2 size={18} /> Clear
                </button>
              )}
              <button 
                className="btn primary" 
                onClick={createProject} 
                disabled={volumes.length === 0 || !outputPath}
              >
                <Plus size={18} /> Add to Queue
              </button>
            </div>
          </div>
          
          {/* Content Area */}
          {volumes.length === 0 ? (
            <div className="empty-state">
              <div className="empty-icon">
                <Film size={40} />
              </div>
              <h3>No BDMV Loaded</h3>
              <p>Load a BDMV folder to start organizing and remuxing your files</p>
              <div className="empty-actions">
                <button className="btn primary" onClick={loadBdmv}>
                  <FolderOpen size={18} /> Load BDMV Folder
                </button>
                <button className="btn secondary" onClick={loadMultipleBdmvs}>
                  <Plus size={18} /> Load Multiple Volumes
                </button>
              </div>
              
              <div className="category-templates">
                <div className="templates-title">Track Layout Templates</div>
                <div className="templates-grid">
                  {(Object.keys(CATEGORIES) as FileCategory[]).map(cat => {
                    const category = CATEGORIES[cat];
                    return (
                      <div 
                        key={cat}
                        className="template-card"
                        onClick={() => setEditingTemplate(cat)}
                      >
                        <div 
                          className="template-card-icon" 
                          style={{ backgroundColor: category.color + '22', color: category.color }}
                        >
                          {cat === 'EPISODE' && <Tv size={20} />}
                          {(cat === 'OP' || cat === 'ED') && <Music size={20} />}
                          {cat === 'SPECIAL' && <Star size={20} />}
                          {cat === 'OTHER' && <FileVideo size={20} />}
                        </div>
                        <h4>{category.label}</h4>
                        <p>{trackTemplates[cat] ? `${trackTemplates[cat].length} tracks` : 'Not configured'}</p>
                      </div>
                    );
                  })}
                </div>
              </div>
            </div>
          ) : (
            <div className="volumes-container">
              <div className="volumes">
                {volumes.map(volume => (
                  <VolumeCard 
                    key={volume.id}
                    volume={volume}
                    isExpanded={expandedVolumes[volume.id] ?? false}
                    onToggle={() => toggleVolumeExpanded(volume.id)}
                    selectedFileId={selectedFileId}
                    onFileSelect={selectFile}
                    onEditFile={(file) => setEditingFile({ file, volumeId: volume.id })}
                  />
                ))}
              </div>
              
              <div className="templates-section">
                <div className="templates-title">Track Layout Templates</div>
                <div className="templates-grid">
                  {(Object.keys(CATEGORIES) as FileCategory[]).map(cat => {
                    const category = CATEGORIES[cat];
                    return (
                      <div 
                        key={cat}
                        className="template-card"
                        onClick={() => setEditingTemplate(cat)}
                      >
                        <div 
                          className="template-card-icon" 
                          style={{ backgroundColor: category.color + '22', color: category.color }}
                        >
                          {cat === 'EPISODE' && <Tv size={20} />}
                          {(cat === 'OP' || cat === 'ED') && <Music size={20} />}
                          {cat === 'SPECIAL' && <Star size={20} />}
                          {cat === 'OTHER' && <FileVideo size={20} />}
                        </div>
                        <h4>{category.label}</h4>
                        <p>{trackTemplates[cat] ? `${trackTemplates[cat].length} tracks` : 'Not configured'}</p>
                      </div>
                    );
                  })}
                </div>
              </div>
            </div>
          )}
        </div>
      </main>
      
      {/* Modals */}
      {editingFile && (
        <TrackEditor 
          file={editingFile.file}
          volumeId={editingFile.volumeId}
          onClose={() => setEditingFile(null)}
        />
      )}
      
      {editingTemplate && (
        <TemplateEditor 
          category={editingTemplate}
          template={trackTemplates[editingTemplate]}
          onClose={() => setEditingTemplate(null)}
        />
      )}
      
      {showSettings && settings && (
        <SettingsModal 
          settings={settings}
          recoveryBackupAvailable={recoveryBackupAvailable}
          settingsSaveAllowed={settingsSaveAllowed}
          recoveryWarning={recoveryWarning}
          onRestoreBackup={previewRecoveryBackup}
          onClose={() => setShowSettings(false)}
        />
      )}

      {(recoveryRestorePreview || recoveryRestoreError || isPreparingRecoveryRestore) && (
        <div className="modal-overlay" onClick={dismissRecoveryRestorePreview}>
          <div
            className="modal settings-modal recovery-restore-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="recovery-restore-title"
            onClick={event => event.stopPropagation()}
          >
            <div className="modal-header">
              <h3 id="recovery-restore-title"><RotateCw size={20} /> Restore Preferences</h3>
              <button
                className="modal-close"
                onClick={dismissRecoveryRestorePreview}
                aria-label="Close restore preview"
              >
                <X size={20} />
              </button>
            </div>
            <div className="modal-body">
              {isPreparingRecoveryRestore && <p>Validating the preserved preferences backup…</p>}
              {recoveryRestoreError && (
                <p className="settings-error" role="alert">{recoveryRestoreError}</p>
              )}
              {recoveryRestorePreview && (
                <>
                  <p>
                    Review the backup before applying it. The current preferences will be replaced;
                    the preserved backup itself will not be changed. Missing or invalid values use
                    defaults.
                  </p>
                  {recoveryRestorePreview.warnings.length > 0 ? (
                    <div className="recovery-restore-warnings" role="status">
                      <strong>Items to review</strong>
                      <ul>
                        {recoveryRestorePreview.warnings.map((warning, index) => (
                          <li key={`${index}-${warning}`}>{warning}</li>
                        ))}
                      </ul>
                    </div>
                  ) : (
                    <p className="recovery-restore-valid" role="status">
                      All saved preference values passed validation.
                    </p>
                  )}
                </>
              )}
            </div>
            <div className="modal-footer">
              <button
                className="btn secondary"
                onClick={dismissRecoveryRestorePreview}
                disabled={isRestoringRecoveryBackup}
              >
                Cancel
              </button>
              {recoveryRestorePreview && (
                <button
                  className="btn primary"
                  onClick={handleRestoreRecoveryBackup}
                  disabled={isRestoringRecoveryBackup}
                >
                  <Check size={16} />
                  {isRestoringRecoveryBackup ? 'Restoring…' : 'Restore Preferences'}
                </button>
              )}
            </div>
          </div>
        </div>
      )}
      
      {/* Loading Overlay */}
      {isLoading && (
        <div className="loading-overlay">
          <div className="loading-spinner" />
          <p>Loading BDMV...</p>
        </div>
      )}
    </div>
  );
};

export default App;
