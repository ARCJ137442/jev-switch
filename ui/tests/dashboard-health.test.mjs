import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';

const { probeProvidersConcurrently } = await loadTs('../src/pages/dashboardHealth.ts');

test('probes in a bounded pool and returns one result per provider', async () => {
  let active = 0;
  let peak = 0;
  const providers = Array.from({ length: 9 }, (_, index) => ({ id: `p${index}`, enabled: true }));
  const health = await probeProvidersConcurrently(providers, async (id) => {
    active++;
    peak = Math.max(peak, active);
    await new Promise((resolve) => setTimeout(resolve, 4));
    active--;
    return { ok: id !== 'p8', latency_ms: 12 };
  }, 4);

  assert.equal(peak, 4);
  assert.equal(Object.keys(health).length, 9);
  assert.deepEqual(health.p0, { state: 'reachable', ms: 12 });
  assert.deepEqual(health.p8, { state: 'unreachable', ms: 12 });
});

test('disabled and failed probes are represented without blocking peers', async () => {
  const health = await probeProvidersConcurrently([
    { id: 'disabled', enabled: false },
    { id: 'failed', enabled: true },
    { id: 'ok', enabled: true },
  ], async (id) => {
    if (id === 'failed') throw new Error('offline');
    return { ok: true, latency_ms: 3 };
  });

  assert.deepEqual(health, {
    disabled: { state: 'disabled', ms: null },
    failed: { state: 'unknown', ms: null },
    ok: { state: 'reachable', ms: 3 },
  });
});

test('rejects an invalid concurrency limit before starting requests', async () => {
  let called = false;
  await assert.rejects(
    probeProvidersConcurrently([{ id: 'p', enabled: true }], async () => {
      called = true;
      return { ok: true, latency_ms: 1 };
    }, 0),
    RangeError,
  );
  assert.equal(called, false);
});
