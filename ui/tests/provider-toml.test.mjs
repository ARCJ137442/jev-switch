import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';
const { parseProviderTomlValue: parse, stripTomlComment: strip } = await loadTs('../src/api/providerTomlValue.ts');
test('provider model arrays preserve commas, hashes, Unicode and escaped quotes in IDs', () => {
  assert.deepEqual(parse('["M1", \'模型,二#号\', "model\\\"three",]'), ['M1', '模型,二#号', 'model"three']);
  assert.deepEqual(parse('[\n "M1",\n "M2",\n]'), ['M1', 'M2']);
  assert.deepEqual(parse('[]'), []);
  assert.equal(strip('api_key = "demo\\\"#still-key" # comment'), 'api_key = "demo\\\"#still-key" ');
});
test('malformed or non-string model arrays are rejected instead of silently imported', () => {
  for (const value of ['[1]', '["M1",, "M2"]', '["unclosed]', '"M1" trailing', '["M1", false]']) assert.equal(parse(value), null, value);
  assert.equal(parse('false'), false); assert.equal(parse('"false"'), 'false');
});
