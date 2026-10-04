import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';

const {
  createDefaultDashboardLayout,
  normalizeDashboardLayout,
  moveDashboardItem,
  updateDashboardItem,
  toggleDashboardItem,
  removeDashboardItem,
  addDashboardItem,
  readDashboardLayout,
  writeDashboardLayout,
} = await loadTs('../src/components/dashboard/layout.ts');

test('default dashboard layout is deterministic and contains one item per registered component', () => {
  const layout = createDefaultDashboardLayout();
  assert.equal(layout.schemaVersion, 1);
  assert.equal(layout.presetId, 'default');
  assert.equal(layout.items.length, 6);
  assert.equal(new Set(layout.items.map(item => item.componentId)).size, layout.items.length);
});

test('normalize drops unknown and duplicate singleton items and repairs widths/order', () => {
  const layout = normalizeDashboardLayout({
    schemaVersion: 99,
    items: [
      { componentId: 'runtime', instanceId: 'one', width: 'invalid' },
      { componentId: 'runtime', instanceId: 'two', width: 'full' },
      { componentId: 'unknown', width: 'half' },
    ],
  });
  assert.equal(layout.items.filter(item => item.componentId === 'runtime').length, 1);
  assert.equal(layout.items.some(item => item.componentId === 'unknown'), false);
  assert.equal(layout.items[0].width, 'half');
  assert.deepEqual(layout.items.map(item => item.order), layout.items.map((_, index) => index));
});

test('move, width, visibility and toggle operations preserve normalized identity', () => {
  const initial = createDefaultDashboardLayout();
  const moved = moveDashboardItem(initial, 'dashboard-runtime', 1);
  assert.equal(moved.items[1].componentId, 'runtime');
  const hidden = updateDashboardItem(moved, 'dashboard-runtime', { visible: false, width: 'wide' });
  assert.equal(hidden.items.find(item => item.componentId === 'runtime').visible, false);
  assert.equal(hidden.items.find(item => item.componentId === 'runtime').width, 'wide');
  const shown = toggleDashboardItem(hidden, 'runtime');
  assert.equal(shown.items.find(item => item.componentId === 'runtime').visible, true);
  const unchanged = toggleDashboardItem(shown, 'missing');
  assert.deepEqual(unchanged, shown);
  const removed = removeDashboardItem(shown, 'dashboard-runtime');
  assert.equal(removed.items.some(item => item.componentId === 'runtime'), false);
  const added = addDashboardItem(removed, 'runtime');
  assert.equal(added.items.some(item => item.componentId === 'runtime'), true);
});

test('layout storage tolerates malformed data and writes a bumped revision', () => {
  const values = new Map([['jev-dashboard-layout-v1', '{broken']]);
  const storage = { getItem: key => values.get(key) ?? null, setItem: (key, value) => values.set(key, value) };
  assert.equal(readDashboardLayout(storage).items.length, 6);
  const saved = writeDashboardLayout(createDefaultDashboardLayout(), storage);
  assert.equal(saved.revision, 1);
  assert.equal(JSON.parse(values.get('jev-dashboard-layout-v1')).revision, 1);
});
