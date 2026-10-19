export function getSettingsLoadUpdate(response) {
  return {
    settings: response.settings,
    recoveryWarning: response.recovery_warning,
    recoveryBackupAvailable: response.recovery_backup_available,
    recoveryBackupOpenError: null,
  };
}

export function getSettingsSaveSuccessUpdate(settings) {
  return {
    settings,
    error: null,
    recoveryWarning: null,
    recoveryBackupAvailable: false,
    recoveryBackupOpenError: null,
  };
}

export function getSettingsSaveFailureUpdate(error) {
  return { error: String(error) };
}

export function getRecoveryWarningDismissUpdate() {
  return { recoveryWarning: null };
}

export function getVisibleNotice(recoveryWarning, error, recoveryBackupOpenError = null) {
  const notice = recoveryWarning || error;
  if (recoveryWarning && recoveryBackupOpenError) {
    return `${recoveryWarning} ${recoveryBackupOpenError}`;
  }
  return notice || recoveryBackupOpenError;
}

export function isRecoveryBackupVisible(recoveryWarning, recoveryBackupAvailable) {
  return Boolean(recoveryWarning && recoveryBackupAvailable);
}
