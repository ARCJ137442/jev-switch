export interface RequestActivityDetail {
  requestId: string | null;
  endpointId: string | null;
  provider: string | null;
  upstreamModel: string | null;
  status: number | null;
  success: boolean | null;
  latencyMs: number | null;
  upstreamCalls: number | null;
  inputTokens: number | null;
  outputTokens: number | null;
  costUsd: number | null;
  attempts: ProviderAttemptDetail[];
}

export interface ProviderAttemptDetail {
  provider: string;
  upstreamModel: string | null;
  attempt: number | null;
  outcome: 'succeeded' | 'failed' | 'cancelled';
  status: number | null;
  latencyMs: number | null;
  retryDecision: string | null;
}

type JsonObject = Record<string, unknown>;

function isObject(value: unknown): value is JsonObject {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function readText(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function readInteger(value: unknown, min = 0, max = Number.MAX_SAFE_INTEGER): number | null {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= min && value <= max
    ? value
    : null;
}

function readCost(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null;
}

export function parseRequestActivityDetail(kind: string, detail: string): RequestActivityDetail | null {
  if (kind !== 'request') return null;

  let value: unknown;
  try {
    value = JSON.parse(detail);
  } catch {
    return null;
  }
  if (!isObject(value)) return null;

  const status = readInteger(value.status, 100, 599);
  const usage = isObject(value.usage) ? value.usage : null;
  const routeTrace = isObject(value.route_trace) ? value.route_trace : null;
  const attempts = Array.isArray(routeTrace?.attempts)
    ? routeTrace.attempts.flatMap((raw): ProviderAttemptDetail[] => {
      if (!isObject(raw)) return [];
      const provider = readText(raw.provider_id);
      const outcome = raw.outcome;
      // A skipped candidate is not an actual provider call.
      if (!provider || (outcome !== 'succeeded' && outcome !== 'failed' && outcome !== 'cancelled')) return [];
      return [{
        provider,
        upstreamModel: readText(raw.upstream_model),
        attempt: readInteger(raw.attempt, 1),
        outcome,
        status: readInteger(raw.upstream_status, 100, 599),
        latencyMs: readInteger(raw.latency_ms),
        retryDecision: readText(raw.retry_decision),
      }];
    })
    : [];
  const success = typeof value.success === 'boolean'
    ? value.success
    : status === null ? null : status >= 200 && status < 300;

  return {
    requestId: readText(value.request_id),
    endpointId: readText(value.endpoint_id),
    provider: readText(value.provider),
    upstreamModel: readText(value.upstream_model),
    status,
    success,
    latencyMs: readInteger(value.latency_ms),
    upstreamCalls: readInteger(value.upstream_calls),
    inputTokens: readInteger(usage?.input_tokens),
    outputTokens: readInteger(usage?.output_tokens),
    costUsd: readCost(value.cost_usd),
    attempts,
  };
}

export type ActivityPerspective = 'entry' | 'provider';

export interface ActivityRow {
  key: string;
  event: ActivityEvent;
  attempt: ProviderAttemptDetail | null;
}

/** Route lifecycle telemetry belongs to the Routing canvas, not call history. */
export function isUserHistoryEvent(event: ActivityEvent): boolean {
  return event.kind !== 'route_activity';
}

/** One durable parent call can yield several actual provider-call rows. */
export function activityRows(events: readonly ActivityEvent[], perspective: ActivityPerspective): ActivityRow[] {
  if (perspective === 'entry') return events.map((event) => ({ key: String(event.id), event, attempt: null }));
  return events.flatMap((event) => {
    const detail = parseRequestActivityDetail(event.kind, event.detail);
    return (detail?.attempts ?? []).map((attempt, index) => ({ key: `${event.id}:${index}`, event, attempt }));
  });
}

export function matchesActivityProvider(event: ActivityEvent, provider: string, perspective: ActivityPerspective): boolean {
  if (provider === 'all') return true;
  const detail = parseRequestActivityDetail(event.kind, event.detail);
  if (!detail) return false;
  if (detail.attempts.length) return detail.attempts.some((attempt) => attempt.provider === provider);
  // Old parent rows lack an attempt trace. Only the entry view may filter by
  // the known selected provider; never invent a provider-call row from it.
  return perspective === 'entry' && detail.provider === provider;
}
import type { ActivityEvent } from '../../api/access';
