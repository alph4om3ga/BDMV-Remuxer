import assert from 'node:assert/strict';
import test from 'node:test';
import { applyToolPathSelection, browseForToolPath } from './toolPathPicker.js';

const settings = {
  mkvmerge_path: '/custom/tools/mkvmerge',
  ffprobe_path: '/custom/tools/ffprobe',
  min_extra_duration: 240,
};

for (const setting of ['mkvmerge_path', 'ffprobe_path']) {
  test(`cancelling the ${setting} picker preserves the manually entered path`, () => {
    const updated = applyToolPathSelection(settings, setting, null);

    assert.strictEqual(updated, settings);
    assert.equal(updated[setting], settings[setting]);
  });
}

for (const setting of ['mkvmerge_path', 'ffprobe_path']) {
  test(`a rejected ${setting} picker preserves the manually entered path and reports the error`, async () => {
    let currentSettings = settings;
    let browseError = null;
    const setSettings = update => {
      currentSettings = update(currentSettings);
    };
    const setError = error => {
      browseError = error;
    };

    await browseForToolPath(
      setting,
      async () => {
        throw new Error('permission denied');
      },
      setSettings,
      setError,
    );

    assert.strictEqual(currentSettings, settings);
    assert.equal(currentSettings[setting], settings[setting]);
    assert.equal(
      browseError,
      'Could not open the file picker: Error: permission denied. Enter the executable path manually or try again.',
    );
  });
}

test('a selected executable replaces only its matching tool path', () => {
  const updated = applyToolPathSelection(settings, 'ffprobe_path', '/picked/ffprobe');

  assert.deepEqual(updated, {
    ...settings,
    ffprobe_path: '/picked/ffprobe',
  });
});
