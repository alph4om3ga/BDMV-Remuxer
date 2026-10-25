export function applyToolPathSelection(settings, setting, selected) {
  if (typeof selected !== 'string') return settings;
  return { ...settings, [setting]: selected };
}
