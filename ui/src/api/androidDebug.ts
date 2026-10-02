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

export function readAndroidDebugLog(): Promise<string> {
  return invoke<string>('android_read_debug_log');
}

export function exportAndroidDebugLog(): Promise<'downloads' | 'shared'> {
  return invoke<'downloads' | 'shared'>('android_export_debug_log');
}

export function recordAndroidWebProbe(ok: boolean, httpStatus: number | null): Promise<void> {
  return invoke<void>('android_record_web_probe', { ok, httpStatus });
}
