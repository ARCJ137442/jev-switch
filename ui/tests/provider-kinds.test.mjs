import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';

const { PROVIDER_KINDS, applyProviderKindPreset, getProviderKindPreset, isProviderKind } =
  await loadTs('../src/api/providerKinds.ts');

test('TypeSafe preset uses the official SystemOne endpoint and documented model ID', () => {
  const preset = getProviderKindPreset('typesafe');
  assert.equal(preset?.defaultBase, 'https://api.typesafe.ai/v1/systemone');
  assert.deepEqual(preset?.defaultModels, ['jev-latest']);
  assert.equal(isProviderKind('typesafe'), true);
});

test('changing kind updates only values that still match the previous preset', () => {
  assert.deepEqual(
    applyProviderKindPreset('laya', 'typesafe', 'http://127.0.0.1:18765/v1/systemone', []),
    { base: 'https://api.typesafe.ai/v1/systemone', models: ['jev-latest'] },
  );
  assert.deepEqual(
    applyProviderKindPreset('laya', 'typesafe', 'https://custom.example/v1/systemone', ['custom-model']),
    { base: 'https://custom.example/v1/systemone', models: ['custom-model'] },
  );
});

test('provider kind options have unique stable values', () => {
  const values = PROVIDER_KINDS.map(({ value }) => value);
  assert.equal(new Set(values).size, values.length);
  assert.equal(isProviderKind('openrouter'), false);
});
