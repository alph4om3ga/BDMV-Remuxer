// ============================================================================
// Track & Media Types
// ============================================================================

export interface Track {
  id: number;
  track_type: 'video' | 'audio' | 'subtitle';
  codec: string;
  language: string;
  name: string;
  default: boolean;
  forced: boolean;
  resolution?: string;
  channels?: string;
}

export interface MediaFile {
  id: string;
  filename: string;
  path: string;
  duration: number;
  size: number;
  category: FileCategory;
  tracks: Track[];
  episode_number?: number;
  op_number?: number;
  ed_number?: number;
  special_number?: number;
  chapter_file?: string;
}

export interface Volume {
  id: string;
  name: string;
  volume_number: number;
  files: MediaFile[];
  probe_failures: ProbeFailure[];
}

export interface ProbeFailure {
  path: string;
  diagnostic: string;
}

// ============================================================================
// Project & Queue Types
// ============================================================================

export type ProjectStatus = 'pending' | 'processing' | 'paused' | 'complete' | 'error';

export interface Project {
  id: string;
  output_name: string;
  output_path: string;
  volumes: Volume[];
  track_templates: Record<string, Track[]>;
  status: ProjectStatus;
  progress: number;
  current_file?: string;
  completed_files: Array<{ path: string; fingerprint: string | null }>;
  created_at: string;
  error?: string | null;
}

// ============================================================================
// Settings
// ============================================================================

export interface Settings {
  mkvmerge_path: string;
  ffprobe_path: string;
  min_episode_duration: number;
  min_extra_duration: number;
  op_ed_min_duration: number;
  op_ed_max_duration: number;
  episode_folder_name: string;
  extra_folder_name: string;
}

// ============================================================================
// Categories
// ============================================================================

export type FileCategory = 'EPISODE' | 'OP' | 'ED' | 'SPECIAL' | 'OTHER';

export interface CategoryDef {
  id: FileCategory;
  label: string;
  color: string;
}

export const CATEGORIES: Record<FileCategory, CategoryDef> = {
  EPISODE: { id: 'EPISODE', label: 'Episode', color: '#4ade80' },
  OP:      { id: 'OP',      label: 'Opening', color: '#60a5fa' },
  ED:      { id: 'ED',      label: 'Ending',  color: '#c084fc' },
  SPECIAL: { id: 'SPECIAL', label: 'Special', color: '#fbbf24' },
  OTHER:   { id: 'OTHER',   label: 'Other',   color: '#94a3b8' },
};

// ============================================================================
// Languages
// ============================================================================

export interface Language {
  code: string;
  name: string;
}

export const LANGUAGES: Language[] = [
  { code: 'und', name: 'Undetermined' },
  { code: 'jpn', name: 'Japanese' },
  { code: 'eng', name: 'English' },
  { code: 'fre', name: 'French' },
  { code: 'ger', name: 'German' },
  { code: 'spa', name: 'Spanish' },
  { code: 'ita', name: 'Italian' },
  { code: 'por', name: 'Portuguese' },
  { code: 'chi', name: 'Chinese' },
  { code: 'kor', name: 'Korean' },
  { code: 'ara', name: 'Arabic' },
  { code: 'rus', name: 'Russian' },
  { code: 'pol', name: 'Polish' },
  { code: 'dut', name: 'Dutch' },
  { code: 'swe', name: 'Swedish' },
  { code: 'nor', name: 'Norwegian' },
  { code: 'dan', name: 'Danish' },
  { code: 'fin', name: 'Finnish' },
  { code: 'hun', name: 'Hungarian' },
  { code: 'cze', name: 'Czech' },
  { code: 'tha', name: 'Thai' },
  { code: 'vie', name: 'Vietnamese' },
];
