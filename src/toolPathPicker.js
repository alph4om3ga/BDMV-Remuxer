export function applyToolPathSelection(settings, setting, selected) {
  if (typeof selected !== 'string') return settings;
  return { ...settings, [setting]: selected };
}

export function getToolPathPickerStartDirectory(configuredPath) {
  if (typeof configuredPath !== 'string' || configuredPath.length === 0) return null;

  const isAbsolute =
    configuredPath.startsWith('/') ||
    /^[a-z]:[\\/]/i.test(configuredPath) ||
    configuredPath.startsWith('\\\\');
  if (!isAbsolute || /[\\/]$/.test(configuredPath)) return null;

  const separatorIndex = Math.max(
    configuredPath.lastIndexOf('/'),
    configuredPath.lastIndexOf('\\'),
  );
  if (separatorIndex < 0) return null;

  if (separatorIndex === 0) return configuredPath[0];
  if (separatorIndex === 2 && /^[a-z]:[\\/]/i.test(configuredPath)) {
    return configuredPath.slice(0, 3);
  }
  return configuredPath.slice(0, separatorIndex);
}

export async function openToolPathPicker(openPicker, title, configuredPath) {
  const options = { title, multiple: false, directory: false };
  const defaultPath = getToolPathPickerStartDirectory(configuredPath);
  if (!defaultPath) return openPicker(options);

  try {
    return await openPicker({ ...options, defaultPath });
  } catch {
    return openPicker(options);
  }
}

export async function browseForToolPath(setting, openPicker, setSettings, setError) {
  try {
    const selected = await openPicker();
    setSettings(current => applyToolPathSelection(current, setting, selected));
    setError(null);
  } catch (error) {
    setError(`Could not open the file picker: ${String(error)}. Enter the executable path manually or try again.`);
  }
}
