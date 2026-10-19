export function getSettingsLoadUpdate(response) {
  return {
    settings: response.settings,
    recoveryWarning: response.recovery_warning,
    recoveryBackupAvailable: response.recovery_backup_available,
    settingsSaveAllowed: response.settings_save_allowed,
    recoveryBackupOpenError: null,
  };
}

export function getSettingsSaveSuccessUpdate(settings, recoveryWarning = null) {
  return {
    settings,
    error: null,
    recoveryWarning,
    recoveryBackupAvailable: false,
    settingsSaveAllowed: true,
    recoveryBackupOpenError: null,
  };
}

export function getSettingsSaveState(settingsSaveAllowed, recoveryWarning) {
  return {
    disabled: !settingsSaveAllowed,
    warning: settingsSaveAllowed
      ? null
      : recoveryWarning ||
        'Settings cannot be saved because the existing preferences could not be safely preserved as a recovery backup.',
  };
}

export function getSettingsSaveFailureUpdate(error) {
  return { error: String(error) };
}

export function getRecoveryWarningDismissUpdate() {
  return { recoveryWarning: null };
}

export function getVisibleNotice(recoveryWarning, error, recoveryBackupOpenError = null) {
  const notices = [recoveryWarning, error, recoveryBackupOpenError]
    .filter(Boolean)
    .filter((notice, index, allNotices) => allNotices.indexOf(notice) === index);
  return notices.length > 0 ? notices.join(' ') : null;
}

export function isRecoveryBackupVisible(recoveryWarning, recoveryBackupAvailable) {
  return Boolean(recoveryWarning && recoveryBackupAvailable);
}
