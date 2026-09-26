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
  };
}
