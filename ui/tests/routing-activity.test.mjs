import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';

const activity = await loadTs('../src/pages/routingActivity.ts');
const routes = [
  { left: 'public', match: 'exact', right: 'fallback' },
  { left: 'fallback', match: 'exact', right: 'provider', upstream_model: 'model-a' },
];
const event = (phase, id = 7) => ({ kind: 'route_activity', id, detail: JSON.stringify({
  phase,
  activity_id: 'request-7',
  requested_model: 'public',
  provider_id: 'provider',
  upstream_model: 'model-a',
  hops: ['fallback', 'provider'],
  route_edges: routes,
}) });

test('correlates started, retry and finished events across every real DAG edge', () => {
  let state = {};
  state = activity.applyRouteActivityEvent(state, routes, event('started'), 1000);
  state = activity.applyRouteActivityEvent(state, routes, event('retrying'), 1200);
  state = activity.applyRouteActivityEvent(state, routes, event('finished_success'), 1500);
  const first = activity.edgeActivityVisual(state[JSON.stringify(['public', 'exact', 'fallback', null])], 1500);
  const second = activity.edgeActivityVisual(state[JSON.stringify(['fallback', 'exact', 'provider', 'model-a'])], 1500);
  assert.equal(first.activeCount, 0);
  assert.equal(second.activeCount, 0);
  assert.equal(first.outcomeCount, 1);
  assert.equal(second.successRate, 1);
  assert(first.glowLevel > 0 && second.glowLevel > 0);
});

test('concurrent activities remain separate and timeout is treated as failure', () => {
  let state = {};
  state = activity.applyRouteActivityEvent(state, routes, event('started', 1), 0);
  state = activity.applyRouteActivityEvent(state, routes, { ...event('started', 2), detail: event('started', 2).detail.replace('request-7', 'request-8') }, 100);
  const timeoutNow = activity.ROUTE_ACTIVITY_TIMEOUT_MS + 101;
  state = activity.advanceRouteActivities(state, timeoutNow);
  const visual = activity.edgeActivityVisual(state[JSON.stringify(['public', 'exact', 'fallback', null])], timeoutNow);
  assert.equal(visual.activeCount, 0);
  assert.equal(visual.outcomeCount, 2);
  assert.equal(visual.successRate, 0);
});

test('health colour uses recent success ratio and settles to neutral with no samples', () => {
  let state = {};
  state = activity.applyRouteActivityEvent(state, routes, event('started'), 1000);
  state = activity.applyRouteActivityEvent(state, routes, event('finished_success'), 1100);
  const healthy = activity.edgeActivityVisual(state[JSON.stringify(['public', 'exact', 'fallback', null])], 1100);
  assert.equal(healthy.hue, 120);
  const idle = activity.edgeActivityVisual(state[JSON.stringify(['public', 'exact', 'fallback', null])], 1000 + activity.ROUTE_ACTIVITY_WINDOW_MS + 2500);
  assert.equal(idle.outcomeCount, 0);
  assert.equal(idle.saturation, 0);
});
