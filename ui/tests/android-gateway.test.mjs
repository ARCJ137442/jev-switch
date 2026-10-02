import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const root = new URL('../', import.meta.url);

test('Android gateway controls keep native plugin IPC behind Rust commands', async () => {
  const api = await readFile(new URL('src/api/androidGateway.ts', root), 'utf8');
  const dashboard = await readFile(new URL('src/components/dashboard/GatewayServiceControl.tsx', root), 'utf8');
  assert.match(api, /android_request_notification_permission/);
  assert.match(api, /android_take_pending_gateway_toggle/);
  assert.doesNotMatch(api, /plugin:jev-android-keepalive\|requestNotificationPermission/);
  assert.doesNotMatch(dashboard, /plugin:jev-android-keepalive\|takePendingGatewayToggle/);
});

test('Android dashboard starts the gateway before requesting optional notification permission', async () => {
  const source = await readFile(new URL('src/components/dashboard/GatewayServiceControl.tsx', root), 'utf8');
  const start = source.indexOf('const status = await setGatewayServiceRunning(next);');
  const permission = source.indexOf('requestAndroidNotificationPermission()', start);
  assert(start >= 0 && permission > start);
});
