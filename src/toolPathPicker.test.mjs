import assert from 'node:assert/strict';
import test from 'node:test';
import { applyToolPathSelection } from './toolPathPicker.js';

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

test('a selected executable replaces only its matching tool path', () => {
  const updated = applyToolPathSelection(settings, 'ffprobe_path', '/picked/ffprobe');

  assert.deepEqual(updated, {
    ...settings,
    ffprobe_path: '/picked/ffprobe',
  });
});
