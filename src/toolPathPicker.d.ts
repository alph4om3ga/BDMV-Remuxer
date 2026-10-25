export type ToolPathSetting = 'mkvmerge_path' | 'ffprobe_path';

export function getToolPathPickerStartDirectory(configuredPath: string): string | null;

export function openToolPathPicker<T extends string | string[] | null>(
  openPicker: (options: {
    title: string;
    multiple: false;
    directory: false;
    defaultPath?: string;
  }) => Promise<T>,
  title: string,
  configuredPath: string,
): Promise<T>;

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
