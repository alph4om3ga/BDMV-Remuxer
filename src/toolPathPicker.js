export function applyToolPathSelection(settings, setting, selected) {
  if (typeof selected !== 'string') return settings;
  return { ...settings, [setting]: selected };
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
