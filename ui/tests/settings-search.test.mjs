import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';

const { settingMatches, highlightSegments } = await loadTs('../src/components/settings/settingsSearch.ts');

test('settings search includes localized descriptions and LAN access labels', () => {
  assert.equal(settingMatches('局域网', 'instance listen', '局域网访问', '允许同一网络设备连接'), true);
  assert.equal(settingMatches('同一网络', 'instance listen', '局域网访问', '允许同一网络设备连接'), true);
  assert.equal(settingMatches('未知词', 'instance listen', '局域网访问'), false);
});

test('highlight keeps exact source text and marks each case-insensitive match', () => {
  assert.deepEqual(highlightSegments('局域网访问：允许局域网设备', '局域网'), [
    { text: '局域网', matched: true }, { text: '访问：允许', matched: false },
    { text: '局域网', matched: true }, { text: '设备', matched: false },
  ]);
  assert.deepEqual(highlightSegments('Gateway API', ''), [{ text: 'Gateway API', matched: false }]);
});
