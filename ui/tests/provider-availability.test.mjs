import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';

const { summarizeProviderAvailability } = await loadTs('../src/pages/providerAvailability.ts');
const event = (id, provider, success) => ({
  id,
  timestamp: id,
  kind: 'request',
  token_id: null,
  detail: JSON.stringify({ provider, success, status: success ? 200 : 503 }),
});

test('provider availability uses the most recent actual request events only', () => {
  const summary = summarizeProviderAvailability(['typesafe', 'laya'], [
    event(5, 'typesafe', true),
    event(4, 'typesafe', false),
    event(3, 'laya', true),
  ], 2);

  assert.deepEqual(summary.typesafe, { samples: ['failure', 'success'], successes: 1, rate: 50 });
  assert.deepEqual(summary.laya, { samples: ['success'], successes: 1, rate: 100 });
});

test('unknown samples remain unknown instead of appearing as zero availability', () => {
  const summary = summarizeProviderAvailability(['missing'], [], 15);
  assert.deepEqual(summary.missing, { samples: [], successes: 0, rate: null });
});

test('malformed and non-request events do not enter the denominator', () => {
  const summary = summarizeProviderAvailability(['typesafe'], [
    { id: 3, timestamp: 3, kind: 'probe', token_id: null, detail: JSON.stringify({ provider: 'typesafe', success: false }) },
    { id: 2, timestamp: 2, kind: 'request', token_id: null, detail: '{bad json' },
    event(1, 'typesafe', true),
  ]);
  assert.deepEqual(summary.typesafe, { samples: ['success'], successes: 1, rate: 100 });
});

test('failed fallback branch counts against its own provider rather than the successful parent', () => {
  const failedOver = {
    ...event(10, 'jev-typesafe', true),
    detail: JSON.stringify({ provider: 'jev-typesafe', success: true, route_trace: { attempts: [
      { provider_id: 'vercel', outcome: 'failed', upstream_status: 401 },
      { provider_id: 'jev-typesafe', outcome: 'succeeded' },
      { provider_id: 'skipped', outcome: 'skipped' },
    ] } }),
  };
  const summary = summarizeProviderAvailability(['vercel', 'jev-typesafe', 'skipped'], [failedOver]);
  assert.deepEqual(summary.vercel, { samples: ['failure'], successes: 0, rate: 0 });
  assert.deepEqual(summary['jev-typesafe'], { samples: ['success'], successes: 1, rate: 100 });
  assert.deepEqual(summary.skipped, { samples: [], successes: 0, rate: null });
});
