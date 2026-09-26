import { getBase } from '../api';
import { adminRequest, getAdminSession } from './admin';
import { getCallerToken } from '../auth/callerSession';
import type { CallerMe } from '../generated/CallerMe';
import type { CallerStats as GeneratedCallerStats } from '../generated/CallerStats';
import type { CallerStatsResponse } from '../generated/CallerStatsResponse';
import type { CreateTokenRequest } from '../generated/CreateTokenRequest';
import type { CreateTokenResponse } from '../generated/CreateTokenResponse';
import type { DeletedToken } from '../generated/DeletedToken';
import type { Event } from '../generated/Event';
import type { EventsPage } from '../generated/EventsPage';
import type { TokenListResponse } from '../generated/TokenListResponse';
import type { TokenRole } from '../generated/TokenRole';
import type { TokenStats as GeneratedTokenStats } from '../generated/TokenStats';
import type { TokenStatsResponse } from '../generated/TokenStatsResponse';
import type { TokenSummary } from '../generated/TokenSummary';
import type { UpdateTokenRequest } from '../generated/UpdateTokenRequest';

/** Caller tokens are distinct from short-lived administrator sessions. */
export type CallerRole = TokenRole;
export type CallerTokenSummary = TokenSummary;
export type ActivityEvent = Event;
export type CallerStats = Omit<GeneratedCallerStats, 'total_cost' | 'avg_latency_ms' | 'error_rate'> & {
  total_cost: number | null;
  avg_latency_ms: number | null;
  error_rate: number | null;
};
export type OwnStats = Omit<GeneratedTokenStats, 'total_cost' | 'avg_latency_ms' | 'error_rate'> & {
  total_cost: number | null;
  avg_latency_ms: number | null;
  error_rate: number | null;
};

export class AccessApiError extends Error {
  readonly status: number;
  constructor(message: string, status: number) {
    super(message);
    this.name = 'AccessApiError';
    this.status = status;
  }
}

function withWindow(from: number, to: number): string {
  const query = new URLSearchParams({ from: String(from), to: String(to) });
  return `?${query.toString()}`;
}

export const createCallerToken = (input: CreateTokenRequest, credential?: string) =>
  managementRequest<CreateTokenResponse>('/v1/admin/tokens', credential, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  });

async function managementRequest<T>(path: string, credential: string | undefined, init?: RequestInit): Promise<T> {
  const callCredential = credential ?? getCallerToken();
  if (callCredential) {
    const headers = new Headers(init?.headers);
    headers.set('Authorization', `Bearer ${callCredential}`);
    const response = await fetch(`${getBase()}${path}`, { ...init, headers });
    const text = await response.text();
    let data: unknown = null;
    try { data = text ? JSON.parse(text) : null; } catch { data = null; }
    if (!response.ok) {
      const message = data && typeof data === 'object' && 'error' in data
        ? String((data as { error: unknown }).error)
        : `HTTP ${response.status}`;
      throw new AccessApiError(message, response.status);
    }
    if (data === null) throw new AccessApiError(`Invalid response (HTTP ${response.status})`, response.status);
    return data as T;
  }
  return adminRequest<T>(path, init);
}

export const listCallerTokens = (credential?: string) => managementRequest<TokenListResponse>('/v1/admin/tokens', credential);

export const updateCallerToken = (id: string, input: UpdateTokenRequest, credential?: string) =>
  managementRequest<{ token: CallerTokenSummary }>(`/v1/admin/tokens/${encodeURIComponent(id)}`, credential, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  });

export const revokeCallerToken = (id: string, credential?: string) =>
  managementRequest<DeletedToken>(`/v1/admin/tokens/${encodeURIComponent(id)}`, credential, { method: 'DELETE' });

export const getCallerStats = async (id: string, from: number, to: number, credential?: string): Promise<OwnStats> =>
  (await managementRequest<TokenStatsResponse>(`/v1/admin/tokens/${encodeURIComponent(id)}/stats${withWindow(from, to)}`, credential)).stats;

export const getAdminStats = (from: number, to: number, credential?: string) =>
  managementRequest<CallerStatsResponse>(`/v1/admin/stats${withWindow(from, to)}`, credential);

export const getAdminActivity = (since = 0, limit = 50, credential?: string) =>
  managementRequest<EventsPage>(`/v1/admin/events?${new URLSearchParams({ since: String(since), limit: String(limit) })}`, credential);

async function callTokenRequest<T>(path: string, token: string): Promise<T> {
  const response = await fetch(`${getBase()}${path}`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  const text = await response.text();
  let data: unknown;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    throw new AccessApiError(`Invalid response (${response.status})`, response.status);
  }
  if (!response.ok) {
    const message = data && typeof data === 'object' && 'error' in data
      ? String((data as { error: unknown }).error)
      : `HTTP ${response.status}`;
    throw new AccessApiError(message, response.status);
  }
  return data as T;
}

export const getMyCallerStats = async (token: string, from: number, to: number): Promise<OwnStats> =>
  (await callTokenRequest<TokenStatsResponse>(`/v1/stats/my${withWindow(from, to)}`, token)).stats;

export const getMyActivity = (token: string, since = 0, limit = 50) =>
  callTokenRequest<EventsPage>(`/v1/events/my?${new URLSearchParams({ since: String(since), limit: String(limit) })}`, token);

export async function getCallerIdentity(token: string): Promise<CallerMe> {
  const result = await callTokenRequest<CallerMe>('/v1/auth/me', token);
  if (!result || typeof result.id !== 'string' || !['admin', 'readonly'].includes(result.role)) {
    throw new AccessApiError('Invalid caller identity response', 502);
  }
  return result;
}

type Scope = { type: 'admin'; credential?: string } | { type: 'caller'; token: string };

/** Fetch-based SSE keeps Authorization in a header; EventSource cannot set one. */
export async function streamActivity(
  scope: Scope,
  since: number,
  signal: AbortSignal,
  onEvent: (event: ActivityEvent) => void,
): Promise<void> {
  const suffix = scope.type === 'admin' ? '/v1/admin/events/stream' : '/v1/events/my/stream';
  const headers = new Headers({ Accept: 'text/event-stream' });
  const token = scope.type === 'admin' ? scope.credential ?? getCallerToken() ?? getAdminSession() : scope.token;
  if (token) headers.set('Authorization', `Bearer ${token}`);
  const response = await fetch(`${getBase()}${suffix}?since=${encodeURIComponent(String(since))}`, { headers, signal });
  if (!response.ok) throw new Error(`Event stream failed (HTTP ${response.status})`);
  if (!response.body) throw new Error('Event stream is not available');

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  try {
    while (!signal.aborted) {
      const { value, done } = await reader.read();
      if (done) return;
      buffer += decoder.decode(value, { stream: true }).replace(/\r\n/g, '\n');
      let boundary = buffer.indexOf('\n\n');
      while (boundary >= 0) {
        const frame = buffer.slice(0, boundary);
        buffer = buffer.slice(boundary + 2);
        const data = frame.split('\n').filter((line) => line.startsWith('data:')).map((line) => line.slice(5).trim()).join('\n');
        if (data) {
          try {
            onEvent(JSON.parse(data) as ActivityEvent);
          } catch {
            // Ignore malformed or non-JSON keepalive frames.
          }
        }
        boundary = buffer.indexOf('\n\n');
      }
    }
  } finally {
    await reader.cancel().catch(() => undefined);
  }
}
