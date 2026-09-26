import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';
const { ComparisonQueue } = await loadTs('../src/components/playground/comparisonQueue.ts');
const tick = () => new Promise(resolve => setImmediate(resolve));
const deferred = () => { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; };

test('bounded queue starts next target only when a slot becomes free; pending cancel never calls', async () => {
  const gates = Array.from({ length: 5 }, deferred); const called = []; const events = [];
  const queue = new ComparisonQueue(3, { onStatus: (job, status) => events.push([job.key, status]) });
  for (let i = 0; i < 5; i++) queue.enqueue({ key: `${i}`, attemptId: `a${i}`, payload: i, execute: () => { called.push(i); return gates[i].promise; } });
  assert.deepEqual(called, [0, 1, 2]); queue.cancel('4', 'a4');
  gates[1].resolve('one'); await tick(); assert.deepEqual(called, [0, 1, 2, 3]);
  for (const gate of gates) gate.resolve('done'); await tick();
  assert.equal(queue.activeCount, 0); assert.equal(queue.pendingCount, 0);
  assert(events.some(([key, status]) => key === '4' && status === 'cancelled'));
  assert(!called.includes(4));
});

test('cancel aborts active fetch and a late old response cannot overwrite a retry', async () => {
  const old = deferred(), fresh = deferred(); const events = []; let signal;
  const queue = new ComparisonQueue(2, { onStatus: (job, status, value) => events.push([job.attemptId, status, value]) });
  queue.enqueue({ key: 'same', attemptId: 'old', payload: null, execute: s => { signal = s; return old.promise; } });
  queue.cancel('same', 'old'); assert(signal.aborted);
  queue.enqueue({ key: 'same', attemptId: 'new', payload: null, execute: () => fresh.promise });
  fresh.resolve('new-result'); old.resolve('old-result'); await tick();
  assert.deepEqual(events.filter(([, status]) => status === 'ok'), [['new', 'ok', 'new-result']]);
});

test('disposing on unmount cancels queued work and a fresh mount can run', async () => {
  const gate = deferred(); let called = 0;
  const queue = new ComparisonQueue(1, { onStatus() {} });
  queue.enqueue({ key: 'a', attemptId: 'a', execute: () => gate.promise });
  queue.enqueue({ key: 'b', attemptId: 'b', execute: async () => { called++; } });
  queue.dispose(); gate.resolve('late'); await tick(); assert.equal(called, 0);
  const fresh = new ComparisonQueue(1, { onStatus() {} });
  fresh.enqueue({ key: 'c', attemptId: 'c', execute: async () => { called++; } });
  await tick(); assert.equal(called, 1);
});
