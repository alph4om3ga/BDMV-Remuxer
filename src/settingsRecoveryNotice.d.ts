import type { Settings } from './types';

interface SettingsResponse {
  settings: Settings;
  recovery_warning: string | null;
  recovery_backup_available: boolean;
}

export function getSettingsLoadUpdate(response: SettingsResponse): {
  settings: Settings;
  recoveryWarning: string | null;
  recoveryBackupAvailable: boolean;
  recoveryBackupOpenError: null;
};
export function getSettingsSaveSuccessUpdate(settings: Settings): {
  settings: Settings;
  error: null;
  recoveryWarning: null;
  recoveryBackupAvailable: false;
  recoveryBackupOpenError: null;
};
export function getSettingsSaveFailureUpdate(error: unknown): { error: string };
export function getRecoveryWarningDismissUpdate(): { recoveryWarning: null };
export function getVisibleNotice(
  recoveryWarning: string | null,
  error: string | null,
  recoveryBackupOpenError?: string | null,
): string | null;
export function isRecoveryBackupVisible(
  recoveryWarning: string | null,
  recoveryBackupAvailable: boolean,
): boolean;
