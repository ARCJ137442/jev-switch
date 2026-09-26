import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';

const session = await loadTs('../src/auth/callerSession.ts');

test('caller credentials stay in memory and identity exposes only id and role', () => {
  const changes = [];
  const unsubscribe = session.subscribeAuth(() => changes.push(session.getAuthIdentity()));
  session.setCallerSession('secret-once', { id: 'team-reader', role: 'readonly' });
  assert.equal(session.getCallerToken(), 'secret-once');
  assert.deepEqual(session.getAuthIdentity(), { kind: 'caller', id: 'team-reader', role: 'readonly' });
  assert.equal(JSON.stringify(session.getAuthIdentity()).includes('secret-once'), false);
  session.clearCallerSession();
  assert.equal(session.getCallerToken(), null);
  assert.equal(session.getAuthIdentity(), null);
  assert.equal(changes.length, 2);
  unsubscribe();
});

test('admin password session role switch clears caller credential without a page reload', () => {
  session.setCallerSession('temporary-admin-call-token', { id: 'ops', role: 'admin' });
  session.markAdminSession();
  assert.equal(session.getCallerToken(), null);
  assert.deepEqual(session.getAuthIdentity(), { kind: 'admin-session', role: 'admin' });
  session.clearCallerSession();
});
