import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';

const { createSettingsBackup, parseSettingsBackup } = await loadTs('../src/settings/backup.ts');

const appSettings = {
  theme: 'system', language: 'zh', autoProviderProbe: false, showStatusBar: true,
  uiScale: 1.1, autoHideRoutingHud: true, apiBase: null,
};
const gateway = {
  schema_version: 1,
  providers: { test: { kind: 'typesafe', base: 'https://example.test/v1/systemone', models: ['jev-latest'], api_key: 'secret-test-key', enabled: true } },
  routes: [{ left: 'test-entry', right: 'test', match: 'exact', priority: 10, sticky: 'none', on_error: 'next' }],
};

test('creates a versioned backup and preserves API keys only in explicit export data', () => {
  const backup = createSettingsBackup(appSettings, gateway, '2026-10-01T00:00:00.000Z');
  assert.equal(backup.format, 'jev-switch-settings');
  assert.equal(backup.schema_version, 1);
  assert.equal(backup.gateway.providers.test.api_key, 'secret-test-key');
  assert.equal(JSON.stringify(backup).includes('admin_password'), false);
});

test('parses a valid backup and rejects malformed or unsupported settings before import', () => {
  const valid = JSON.stringify(createSettingsBackup(appSettings, gateway));
  assert.deepEqual(parseSettingsBackup(valid).app_settings, appSettings);
  assert.throws(() => parseSettingsBackup('{'), /invalid JSON/);
  assert.throws(() => parseSettingsBackup(JSON.stringify({ format: 'jev-switch-settings', schema_version: 99 })), /unsupported/);
  assert.throws(() => parseSettingsBackup(JSON.stringify({ ...JSON.parse(valid), app_settings: { ...appSettings, uiScale: 3 } })), /UI scale/);
  const invalidGateway = { ...gateway, providers: { test: { kind: 'laya', base: '', models: [4], enabled: true } } };
  assert.throws(() => parseSettingsBackup(JSON.stringify({ ...JSON.parse(valid), gateway: invalidGateway })), /invalid provider/);
});

test('routing HUD auto-hide defaults on and persists explicit changes', async () => {
  const preferences = await loadTs('../src/settings/preferences.ts');
  const values = new Map();
  const storage = {
    getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  };
  assert.equal(preferences.readRoutingHudAutoHide(storage), true);
  preferences.writeRoutingHudAutoHide(false, storage);
  assert.equal(preferences.readRoutingHudAutoHide(storage), false);
  preferences.writeRoutingHudAutoHide(true, storage);
  assert.equal(preferences.readRoutingHudAutoHide(storage), true);
});
