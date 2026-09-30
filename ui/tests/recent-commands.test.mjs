import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';
const { updateRecentCommands, RECENT_COMMANDS_LIMIT } = await loadTs('../src/commands/recent.ts');

test('recent commands move to the front without duplicate entries and remain bounded', () => {
  assert.deepEqual(updateRecentCommands(['a', 'b', 'c'], 'b'), ['b', 'a', 'c']);
  assert.equal(updateRecentCommands(Array.from({ length: 30 }, (_, index) => String(index)), 'new').length, RECENT_COMMANDS_LIMIT);
});
