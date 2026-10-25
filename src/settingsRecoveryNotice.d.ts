import type { Settings } from './types';

interface SettingsResponse {
  settings: Settings;
  recovery_warning: string | null;
  recovery_backup_available: boolean;
  settings_save_allowed: boolean;
  recovery_retry_available: boolean;
}

export function getSettingsLoadUpdate(response: SettingsResponse): {
  settings: Settings;
  recoveryWarning: string | null;
  recoveryBackupAvailable: boolean;
  settingsSaveAllowed: boolean;
  recoveryRetryAvailable: boolean;
  recoveryBackupOpenError: null;
};
export function getSettingsSaveSuccessUpdate(
  settings: Settings,
  recoveryWarning?: string | null,
): {
  settings: Settings;
  error: null;
  recoveryWarning: string | null;
  recoveryBackupAvailable: false;
  settingsSaveAllowed: true;
  recoveryRetryAvailable: false;
  recoveryBackupOpenError: null;
};
export function getSettingsSaveState(
  settingsSaveAllowed: boolean,
  recoveryWarning: string | null,
): { disabled: boolean; warning: string | null };
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
