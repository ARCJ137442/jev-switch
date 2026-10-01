import { invoke, isTauri } from '@tauri-apps/api/core';

export interface AndroidGatewayStatus {
  running: boolean;
  bind: string | null;
  desired_running: boolean;
}

export interface AndroidKeepaliveNotificationStatus {
  enabled: boolean;
}

export function isAndroidTauri(): boolean {
  return isTauri() && /Android/i.test(navigator.userAgent);
}

export async function getAndroidGatewayStatus(): Promise<AndroidGatewayStatus | null> {
  if (!isAndroidTauri()) return null;
  return invoke<AndroidGatewayStatus>('gateway_status');
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

export async function getAndroidNotificationPermissionState(): Promise<string> {
  const result = await invoke<{ state: string }>('android_notification_permission_state');
  return result.state;
}

export async function requestAndroidNotificationPermission(): Promise<string> {
  const result = await invoke<{ state: string }>('plugin:jev-android-keepalive|requestNotificationPermission');
  return result.state;
}
