import assert from 'node:assert/strict';
import test from 'node:test';
import {
  getRecoveryWarningDismissUpdate,
  getSettingsLoadUpdate,
  getSettingsSaveFailureUpdate,
  getSettingsSaveState,
  getSettingsSaveSuccessUpdate,
  getVisibleNotice,
  isRecoveryBackupVisible,
} from './settingsRecoveryNotice.js';

const settings = { ffprobe_path: '/tools/ffprobe' };
const backupPath = '/home/user/.config/bdmv-remuxer/settings.json.corrupt';
const recoveryWarning = `Saved preferences could not be loaded. Defaults are active. The damaged file was preserved at ${backupPath}.`;

function applyUpdate(state, update) {
  return { ...state, ...update };
}

test('malformed settings response shows the recovery warning and its backup action', () => {
  const response = {
    settings,
    recovery_warning: recoveryWarning,
    recovery_backup_available: true,
    settings_save_allowed: true,
  };
  const state = applyUpdate({
    error: null,
    recoveryWarning: null,
    recoveryBackupAvailable: false,
  }, getSettingsLoadUpdate(response));

  assert.equal(getVisibleNotice(state.recoveryWarning, state.error), recoveryWarning);
  assert.match(getVisibleNotice(state.recoveryWarning, state.error), new RegExp(backupPath.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
  assert.equal(isRecoveryBackupVisible(state.recoveryWarning, state.recoveryBackupAvailable), true);
});

test('the recovery warning remains visible after a failed settings save', () => {
  const recovered = applyUpdate({
    error: null,
    recoveryWarning: null,
    recoveryBackupAvailable: false,
  }, getSettingsLoadUpdate({
    settings,
    recovery_warning: recoveryWarning,
    recovery_backup_available: true,
    settings_save_allowed: true,
  }));
  const afterFailedSave = applyUpdate(recovered, getSettingsSaveFailureUpdate('settings are read-only'));

  assert.equal(afterFailedSave.error, 'settings are read-only');
  assert.equal(
    getVisibleNotice(afterFailedSave.recoveryWarning, afterFailedSave.error),
    `${recoveryWarning} settings are read-only`,
  );
  assert.equal(isRecoveryBackupVisible(afterFailedSave.recoveryWarning, afterFailedSave.recoveryBackupAvailable), true);
});

test('a failed backup-open action is visible without hiding recovery guidance', () => {
  const openError = 'Could not open the recovered preferences backup: access was denied.';

  assert.equal(
    getVisibleNotice(recoveryWarning, null, openError),
    `${recoveryWarning} ${openError}`,
  );
  assert.equal(
    getVisibleNotice(recoveryWarning, 'settings are read-only', openError),
    `${recoveryWarning} settings are read-only ${openError}`,
  );
  assert.equal(getVisibleNotice(null, null, openError), openError);
});

test('dismissing the warning hides it without discarding the backup availability', () => {
  const state = applyUpdate({
    error: null,
    recoveryWarning,
    recoveryBackupAvailable: true,
  }, getRecoveryWarningDismissUpdate());

  assert.equal(getVisibleNotice(state.recoveryWarning, state.error), null);
  assert.equal(isRecoveryBackupVisible(state.recoveryWarning, state.recoveryBackupAvailable), false);
  assert.equal(state.recoveryBackupAvailable, true);
});

test('a successful settings save clears the recovery warning and backup action', () => {
  const state = applyUpdate({
    error: 'previous save failed',
    recoveryWarning,
    recoveryBackupAvailable: true,
  }, getSettingsSaveSuccessUpdate(settings));

  assert.equal(state.settings, settings);
  assert.equal(getVisibleNotice(state.recoveryWarning, state.error), null);
  assert.equal(isRecoveryBackupVisible(state.recoveryWarning, state.recoveryBackupAvailable), false);
  assert.equal(state.settingsSaveAllowed, true);
});

test('settings save is disabled with the recovery explanation when preservation failed', () => {
  const warning = 'A recovery copy could not be created because permission was denied.';
  const state = applyUpdate({
    error: null,
    recoveryWarning: null,
    recoveryBackupAvailable: false,
    settingsSaveAllowed: true,
  }, getSettingsLoadUpdate({
    settings,
    recovery_warning: warning,
    recovery_backup_available: false,
    settings_save_allowed: false,
  }));
  const saveState = getSettingsSaveState(state.settingsSaveAllowed, state.recoveryWarning);

  assert.equal(state.settingsSaveAllowed, false);
  assert.equal(saveState.disabled, true);
  assert.equal(saveState.warning, warning);
});

test('settings save is enabled when the damaged preferences were preserved', () => {
  const saveState = getSettingsSaveState(true, recoveryWarning);

  assert.equal(saveState.disabled, false);
  assert.equal(saveState.warning, null);
});
