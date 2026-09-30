import { isTauri } from '@tauri-apps/api/core';
import { getGatewayControl, putGatewayControl } from './admin';
import { getAndroidGatewayStatus, isAndroidTauri, startAndroidGateway, stopAndroidGateway } from './androidGateway';

export interface GatewayServiceStatus {
  running: boolean;
  bind: string | null;
}

export async function getGatewayServiceStatus(): Promise<GatewayServiceStatus | null> {
  if (!isTauri()) return null;
  if (isAndroidTauri()) {
    const status = await getAndroidGatewayStatus();
    return status ? { running: status.running, bind: status.bind } : null;
  }
  const status = await getGatewayControl();
  return { running: status.running, bind: null };
}

export async function setGatewayServiceRunning(running: boolean): Promise<GatewayServiceStatus> {
  if (!isTauri()) throw new Error('Gateway control is available only in the Tauri app.');
  if (isAndroidTauri()) {
    const status = running ? await startAndroidGateway() : await stopAndroidGateway();
    return { running: status.running, bind: status.bind };
  }
  const status = await putGatewayControl(running);
  return { running: status.running, bind: null };
}
