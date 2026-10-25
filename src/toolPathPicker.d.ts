export type ToolPathSetting = 'mkvmerge_path' | 'ffprobe_path';

export function applyToolPathSelection<T extends Record<ToolPathSetting, string>>(
  settings: T,
  setting: ToolPathSetting,
  selected: string | string[] | null,
): T;
