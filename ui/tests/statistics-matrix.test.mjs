import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';
const { buildHealthRows, buildAdaptiveHealthRows, selectHeatmapGranularity } = await loadTs('../src/pages/statisticsMatrix.ts');

test('daily health matrix uses request outcomes and leaves empty days unknown', () => {
  const day = Date.UTC(2026, 8, 30, 12);
  const events = [
    { timestamp: day, kind: 'request', detail: JSON.stringify({ endpoint_id: 'jev', provider: 'laya', success: true }) },
    { timestamp: day + 1, kind: 'request', detail: JSON.stringify({ endpoint_id: 'jev', provider: 'laya', success: false }) },
    { timestamp: day + 2, kind: 'probe', detail: JSON.stringify({ provider: 'other', success: true }) },
  ];
  const [row] = buildHealthRows(events, 'entry', day, 2);
  assert.equal(row.label, 'jev');
  assert.deepEqual(row.cells.map((cell) => [cell.count, cell.rate]), [[0, null], [2, 0.5]]);
  assert.deepEqual(buildHealthRows(events, 'provider', day, 2).map((item) => item.label), ['laya']);
});

test('adaptive health matrix follows the selected time range and picks the finest square-cell scale', () => {
  const start = Date.UTC(2026, 8, 30, 12, 0, 0);
  const end = start + 10 * 60 * 1000;
  assert.equal(selectHeatmapGranularity(start, end, 900), 'minute');
  const events = [
    { timestamp: start + 5 * 60 * 1000, kind: 'request', detail: JSON.stringify({ endpoint_id: 'jev', provider: 'laya', success: true }) },
    { timestamp: end + 1000, kind: 'request', detail: JSON.stringify({ endpoint_id: 'jev', provider: 'laya', success: false }) },
  ];
  const matrix = buildAdaptiveHealthRows(events, 'entry', start, end, 900);
  assert.equal(matrix.granularity, 'minute');
  assert.equal(matrix.rows[0].cells.reduce((sum, cell) => sum + cell.count, 0), 1);
  assert.equal(matrix.rows[0].cells.filter((cell) => cell.count > 0).length, 1);
});

test('provider heatmap counts each dispatched attempt while entry heatmap counts one client request', () => {
  const at = Date.UTC(2026, 9, 3, 10);
  const events = [{ timestamp: at, kind: 'request', detail: JSON.stringify({
    endpoint_id: 'jev-vercel', provider: 'jev-typesafe', success: true,
    route_trace: { attempts: [
      { provider_id: 'vercel', outcome: 'failed' },
      { provider_id: 'jev-typesafe', outcome: 'succeeded' },
    ] },
  }) }];
  const entry = buildAdaptiveHealthRows(events, 'entry', at, at, 900);
  assert.deepEqual(entry.rows.map((row) => [row.id, row.cells[0].success, row.cells[0].failure]), [['jev-vercel', 1, 0]]);
  const provider = buildAdaptiveHealthRows(events, 'provider', at, at, 900);
  assert.deepEqual(provider.rows.map((row) => [row.id, row.cells[0].success, row.cells[0].failure]), [['jev-typesafe', 1, 0], ['vercel', 0, 1]]);
  assert.deepEqual(buildAdaptiveHealthRows(events, 'provider', at, at, 900, 'vercel').rows.map((row) => row.id), ['vercel']);
});
