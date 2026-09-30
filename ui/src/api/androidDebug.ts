import { invoke, isTauri } from '@tauri-apps/api/core';

export interface AndroidDebugLogStatus { enabled: boolean; path: string | null }

export function isAndroidTauriRuntime(): boolean {
  return isTauri() && /Android/i.test(navigator.userAgent);
}

export async function getAndroidDebugLogStatus(): Promise<AndroidDebugLogStatus | null> {
  if (!isAndroidTauriRuntime()) return null;
  return invoke<AndroidDebugLogStatus>('android_debug_log_status');
}

export async function setAndroidDebugLog(enabled: boolean): Promise<AndroidDebugLogStatus> {
  return invoke<AndroidDebugLogStatus>('android_set_debug_log', { enabled });
}
