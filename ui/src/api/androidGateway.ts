import { invoke, isTauri } from '@tauri-apps/api/core';

export interface AndroidGatewayStatus {
  running: boolean;
  bind: string | null;
  desired_running: boolean;
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
