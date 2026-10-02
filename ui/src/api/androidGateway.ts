import { invoke, isTauri } from '@tauri-apps/api/core';

export interface AndroidGatewayStatus {
  running: boolean;
  bind: string | null;
  desired_running: boolean;
}

export interface AndroidGatewayProbe {
  running: boolean;
  bind: string | null;
  http_status: number | null;
  identity_ok: boolean;
  error: string | null;
}

export interface AndroidKeepaliveNotificationStatus {
  enabled: boolean;
}

export type AndroidNotificationPermissionState = 'granted' | 'denied' | 'prompt' | 'unknown';

function normalizeNotificationPermissionState(value: unknown): AndroidNotificationPermissionState {
  return value === 'granted' || value === 'denied' || value === 'prompt' ? value : 'unknown';
}

export function isAndroidTauri(): boolean {
  return isTauri() && /Android/i.test(navigator.userAgent);
}

export async function getAndroidGatewayStatus(): Promise<AndroidGatewayStatus | null> {
  if (!isAndroidTauri()) return null;
  return invoke<AndroidGatewayStatus>('gateway_status');
}

export function probeAndroidGateway(): Promise<AndroidGatewayProbe> {
  return invoke<AndroidGatewayProbe>('android_gateway_probe');
}

export async function startAndroidGateway(): Promise<AndroidGatewayStatus> {
  return invoke<AndroidGatewayStatus>('start_gateway');
}

export async function stopAndroidGateway(): Promise<AndroidGatewayStatus> {
  return invoke<AndroidGatewayStatus>('stop_gateway');
}

export async function toggleAndroidGateway(): Promise<AndroidGatewayStatus> {
  return invoke<AndroidGatewayStatus>('toggle_gateway');
}

export async function getAndroidKeepaliveNotificationStatus(): Promise<AndroidKeepaliveNotificationStatus> {
  return invoke<AndroidKeepaliveNotificationStatus>('android_keepalive_notification_status');
}

export async function setAndroidKeepaliveNotification(enabled: boolean): Promise<AndroidKeepaliveNotificationStatus> {
  return invoke<AndroidKeepaliveNotificationStatus>('android_set_keepalive_notification', { enabled });
}

export async function getAndroidNotificationPermissionState(): Promise<AndroidNotificationPermissionState> {
  const result = await invoke<{ state: string }>('android_notification_permission_state');
  return normalizeNotificationPermissionState(result?.state);
}

export async function requestAndroidNotificationPermission(): Promise<AndroidNotificationPermissionState> {
  return normalizeNotificationPermissionState(await invoke<string>('android_request_notification_permission'));
}

export async function takePendingAndroidGatewayToggle(): Promise<boolean> {
  return invoke<boolean>('android_take_pending_gateway_toggle');
}
