import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';
const { resolveApiBase } = await loadTs('../src/api/base.ts');
const developmentBase = 'http://127.0.0.1:11435';

test('deployed UI uses its own origin for custom ports and HTTPS reverse proxies', () => {
  for (const url of ['http://127.0.0.1:11436/', 'http://gateway.example:18080/', 'https://gateway.example/']) {
    assert.equal(resolveApiBase(url, false, undefined, developmentBase), '', url);
  }
});

test('Vite development and explicit runtime API overrides remain supported', () => {
  assert.equal(resolveApiBase('http://127.0.0.1:5173/', true, undefined, developmentBase), developmentBase);
  assert.equal(resolveApiBase('https://console.example/', false, 'https://gateway.example', developmentBase), 'https://gateway.example');
  assert.equal(resolveApiBase(undefined, false, undefined, developmentBase), developmentBase);
  assert.equal(resolveApiBase('tauri://localhost/', false, undefined, developmentBase), developmentBase);
});
