export function getSettingsLoadUpdate(response) {
  return {
    settings: response.settings,
    recoveryWarning: response.recovery_warning,
    recoveryBackupAvailable: response.recovery_backup_available,
  };
}

export function getSettingsSaveSuccessUpdate(settings) {
  return {
    settings,
    error: null,
    recoveryWarning: null,
    recoveryBackupAvailable: false,
  };
}

export function getSettingsSaveFailureUpdate(error) {
  return { error: String(error) };
}

export function getRecoveryWarningDismissUpdate() {
  return { recoveryWarning: null };
}

export function getVisibleNotice(recoveryWarning, error) {
  return recoveryWarning || error;
}

export function isRecoveryBackupVisible(recoveryWarning, recoveryBackupAvailable) {
  return Boolean(recoveryWarning && recoveryBackupAvailable);
}
