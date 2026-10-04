import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';

const { parseRequestActivityDetail } = await loadTs('../src/components/access/activityDetail.ts');

test('parses a request event into readable route and result fields', () => {
  const detail = parseRequestActivityDetail('request', JSON.stringify({
    request_id: 'jev-42',
    endpoint_id: 'laya-english',
    provider: 'laya',
    upstream_model: 'laya-english',
    status: 200,
    success: true,
    latency_ms: 205,
    upstream_calls: 1,
    usage: { input_tokens: 46, output_tokens: 0 },
    cost_usd: null,
  }));

  assert.deepEqual(detail, {
    requestId: 'jev-42',
    endpointId: 'laya-english',
    provider: 'laya',
    upstreamModel: 'laya-english',
    status: 200,
    success: true,
    latencyMs: 205,
    upstreamCalls: 1,
    inputTokens: 46,
    outputTokens: 0,
    costUsd: null,
    attempts: [],
  });
});

test('preserves zero values and infers failure from an HTTP error status', () => {
  const detail = parseRequestActivityDetail('request', JSON.stringify({
    status: 503,
    latency_ms: 0,
    usage: { input_tokens: 0, output_tokens: 0 },
    cost_usd: 0,
  }));

  assert.equal(detail?.success, false);
  assert.equal(detail?.latencyMs, 0);
  assert.equal(detail?.inputTokens, 0);
  assert.equal(detail?.outputTokens, 0);
  assert.equal(detail?.costUsd, 0);
});

test('does not treat malformed JSON or unrelated event kinds as request summaries', () => {
  assert.equal(parseRequestActivityDetail('request', '{not json'), null);
  assert.equal(parseRequestActivityDetail('config_change', '{"endpoint_id":"jev"}'), null);
});

test('ignores incorrectly typed fields instead of rendering misleading values', () => {
  const detail = parseRequestActivityDetail('request', JSON.stringify({
    endpoint_id: 17,
    provider: '  ',
    status: '200',
    success: 'yes',
    latency_ms: -1,
    upstream_calls: 1.5,
    usage: { input_tokens: '46', output_tokens: -3 },
    cost_usd: '0.1',
  }));

  assert.deepEqual(detail, {
    requestId: null,
    endpointId: null,
    provider: null,
    upstreamModel: null,
    status: null,
    success: null,
    latencyMs: null,
    upstreamCalls: null,
    inputTokens: null,
    outputTokens: null,
    costUsd: null,
    attempts: [],
  });
});

test('route lifecycle telemetry stays out of user call history', async () => {
  const { isUserHistoryEvent } = await loadTs('../src/components/access/activityDetail.ts');
  assert.equal(isUserHistoryEvent({ kind: 'request' }), true);
  assert.equal(isUserHistoryEvent({ kind: 'route_activity' }), false);
});

test('one durable entry call yields its two dispatched provider attempts', async () => {
  const { activityRows, matchesActivityProvider } = await loadTs('../src/components/access/activityDetail.ts');
  const event = {
    id: 147, timestamp: 1, kind: 'request', token_id: null,
    detail: JSON.stringify({ request_id: 'jev-147', endpoint_id: 'jev-vercel', provider: 'jev-typesafe', success: true,
      route_trace: { attempts: [
        { provider_id: 'vercel', upstream_model: 'typesafe-ai/jev', attempt: 1, outcome: 'failed', upstream_status: 401, latency_ms: 114, retry_decision: 'next_candidate' },
        { provider_id: 'jev-typesafe', upstream_model: 'jev-latest', attempt: 1, outcome: 'succeeded', latency_ms: 902 },
        { provider_id: 'missing', outcome: 'skipped', reason: 'capability_mismatch' },
      ] },
    }),
  };
  assert.equal(activityRows([event], 'entry').length, 1);
  const rows = activityRows([event], 'provider');
  assert.equal(rows.length, 2);
  assert.deepEqual(rows.map((row) => [row.key, row.attempt.provider, row.attempt.outcome, row.attempt.status]), [
    ['147:0', 'vercel', 'failed', 401], ['147:1', 'jev-typesafe', 'succeeded', null],
  ]);
  assert.equal(matchesActivityProvider(event, 'vercel', 'provider'), true);
  assert.equal(matchesActivityProvider(event, 'vercel', 'entry'), true);
});
