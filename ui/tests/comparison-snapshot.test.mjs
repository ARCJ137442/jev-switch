import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';
const { getSnapshot } = await loadTs('../src/components/playground/comparisonSnapshot.ts');
const questions = JSON.stringify({ check: { type: 'noul', instructions: 'Check', criteria: { true: 'yes', false: 'no' } } });

test('snapshot parsing identifies the invalid field and permits explicit null state', () => {
  assert.throws(() => getSnapshot('', questions), /^Error: state$/);
  for (const invalid of ['', '{', 'null', '[]', '{}']) {
    assert.throws(() => getSnapshot('null', invalid), /^Error: questions$/);
  }
  const snapshot = getSnapshot('null', questions);
  assert.equal(snapshot.state, null);
  assert.equal(snapshot.questions.check.type, 'noul');
});

test('input snapshots remain unchanged when the editor input changes', () => {
  const first = getSnapshot('{"value":1}', questions);
  const second = getSnapshot('{"value":2}', questions);
  assert.equal(first.state.value, 1);
  assert.notEqual(first.fingerprint, second.fingerprint);
  assert.equal(first.fingerprint, getSnapshot('{ "value": 1 }', questions).fingerprint);
});
