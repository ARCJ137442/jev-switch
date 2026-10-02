import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';
const { selectExample } = await loadTs('../src/components/playground/sampleSelection.ts');

test('tap toggles with one selected; context action focuses one example', () => {
  assert.deepEqual(selectExample(['a', 'b'], 'c', false), ['a', 'b', 'c']);
  assert.deepEqual(selectExample(['a'], 'b', false), ['a', 'b']);
  assert.deepEqual(selectExample(['a', 'b'], 'b', false), ['a']);
  assert.deepEqual(selectExample(['a'], 'a', false), ['a']);
  assert.deepEqual(selectExample(['a', 'b'], 'b', true), ['b']);
});
