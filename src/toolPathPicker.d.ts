export type ToolPathSetting = 'mkvmerge_path' | 'ffprobe_path';

export function applyToolPathSelection<T extends Record<ToolPathSetting, string>>(
  settings: T,
  setting: ToolPathSetting,
  selected: string | string[] | null,
): T;

export function browseForToolPath<T extends Record<ToolPathSetting, string>>(
  setting: ToolPathSetting,
  openPicker: () => Promise<string | string[] | null>,
  setSettings: (update: (current: T) => T) => void,
  setError: (error: string | null) => void,
): Promise<void>;
