import assert from 'node:assert/strict';
import test from 'node:test';
import {
  applyToolPathSelection,
  browseForToolPath,
  getToolPathPickerStartDirectory,
  openToolPathPicker,
} from './toolPathPicker.js';

const settings = {
  mkvmerge_path: '/custom/tools/mkvmerge',
  ffprobe_path: '/custom/tools/ffprobe',
  min_extra_duration: 240,
};

test('the picker starts in the configured executable folder on Unix paths', () => {
  assert.equal(getToolPathPickerStartDirectory('/custom/tools/mkvmerge'), '/custom/tools');
  assert.equal(getToolPathPickerStartDirectory('/mkvmerge'), '/');
});

test('the picker starts in the configured executable folder on Windows paths', () => {
  assert.equal(getToolPathPickerStartDirectory('C:\\Custom Tools\\mkvmerge.exe'), 'C:\\Custom Tools');
  assert.equal(getToolPathPickerStartDirectory('C:\\mkvmerge.exe'), 'C:\\');
  assert.equal(
    getToolPathPickerStartDirectory('\\\\server\\share\\tools\\ffprobe.exe'),
    '\\\\server\\share\\tools',
  );
});

test('default commands and non-absolute or incomplete paths have no starting folder', () => {
  for (const path of ['mkvmerge', 'ffprobe', 'tools/ffprobe', '', '/custom/tools/']) {
    assert.equal(getToolPathPickerStartDirectory(path), null);
  }
});

test('the picker receives the configured executable folder as its starting location', async () => {
  let pickerOptions;
  const selected = await openToolPathPicker(async options => {
    pickerOptions = options;
    return '/custom/tools/new-mkvmerge';
  }, 'Select mkvmerge executable', '/custom/tools/mkvmerge');

  assert.equal(selected, '/custom/tools/new-mkvmerge');
  assert.deepEqual(pickerOptions, {
    title: 'Select mkvmerge executable',
    multiple: false,
    directory: false,
    defaultPath: '/custom/tools',
  });
});

test('an unavailable starting folder retries the picker without a default path', async () => {
  const calls = [];
  const selected = await openToolPathPicker(async options => {
    calls.push(options);
    if (options.defaultPath) throw new Error('folder is unavailable');
    return '/picked/ffprobe';
  }, 'Select ffprobe executable', '/missing/tools/ffprobe');

  assert.equal(selected, '/picked/ffprobe');
  assert.equal(calls.length, 2);
  assert.equal(calls[0].defaultPath, '/missing/tools');
  assert.deepEqual(calls[1], {
    title: 'Select ffprobe executable',
    multiple: false,
    directory: false,
  });
});

test('cancelling the starting-folder picker does not trigger a fallback picker', async () => {
  let pickerCalls = 0;
  const selected = await openToolPathPicker(async () => {
    pickerCalls += 1;
    return null;
  }, 'Select ffprobe executable', '/custom/tools/ffprobe');

  assert.equal(selected, null);
  assert.equal(pickerCalls, 1);
});

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
