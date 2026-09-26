export type CallerRole = 'admin' | 'readonly';

export interface CallerIdentity {
  id: string;
  role: CallerRole;
}

export type AuthIdentity =
  | { kind: 'caller'; id: string; role: CallerRole }
  | { kind: 'admin-session'; role: 'admin' }
  | null;

let callerToken: string | null = null;
let identity: AuthIdentity = null;
const listeners = new Set<() => void>();

export function getCallerToken(): string | null {
  return callerToken;
}

export function getAuthIdentity(): AuthIdentity {
  return identity;
}

export function setCallerSession(token: string, caller: CallerIdentity): void {
  callerToken = token;
  identity = { kind: 'caller', id: caller.id, role: caller.role };
  notify();
}

export function clearCallerSession(): void {
  callerToken = null;
  if (identity?.kind === 'caller') identity = null;
  notify();
}

export function markAdminSession(): void {
  callerToken = null;
  identity = { kind: 'admin-session', role: 'admin' };
  notify();
}

function notify(): void {
  for (const listener of listeners) listener();
}

export function subscribeAuth(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
