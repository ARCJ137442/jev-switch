import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';
const { selectExample } = await loadTs('../src/components/playground/sampleSelection.ts');

test('ordinary selection is exclusive and shift-click toggles while preserving one selection', () => {
  assert.deepEqual(selectExample(['a', 'b'], 'c', false), ['c']);
  assert.deepEqual(selectExample(['a'], 'b', true), ['a', 'b']);
  assert.deepEqual(selectExample(['a', 'b'], 'b', true), ['a']);
  assert.deepEqual(selectExample(['a'], 'a', true), ['a']);
});
