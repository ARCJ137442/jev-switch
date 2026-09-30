import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';
const { buildTrafficWindow, TRAFFIC_WINDOW_MS } = await loadTs('../src/pages/telemetryWindow.ts');

test('traffic series has a fixed ten-minute evenly spaced axis and zero-fills missing samples', () => {
  const end = 900_000;
  const points = buildTrafficWindow([{ sample_at_ms: end - 210_000, ingress_bps: 8, egress_bps: 4 }], end, 4);
  assert.equal(points.length, 4);
  assert.equal(points[0].sample_at_ms, end - TRAFFIC_WINDOW_MS);
  assert.equal(points.at(-1).sample_at_ms, end);
  assert.deepEqual(points.map((point) => point.ingress_bps), [0, 0, 8, 8]);
  assert.equal(points[1].sample_at_ms - points[0].sample_at_ms, TRAFFIC_WINDOW_MS / 3);
});

test('empty and stale traffic data render as zero', () => {
  const points = buildTrafficWindow([{ sample_at_ms: 1, ingress_bps: 9, egress_bps: 7 }], 900_000, 3);
  assert.deepEqual(points.map(({ ingress_bps, egress_bps }) => [ingress_bps, egress_bps]), [[0, 0], [0, 0], [0, 0]]);
});
