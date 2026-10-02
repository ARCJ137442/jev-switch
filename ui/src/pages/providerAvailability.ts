import type { ActivityEvent } from '../api/access';

export interface ProviderAvailability {
  samples: Array<'success' | 'failure'>;
  successes: number;
  rate: number | null;
}

function readProviderSamples(event: ActivityEvent): Array<{ provider: string; success: boolean }> {
  if (event.kind !== 'request') return [];
  let value: unknown;
  try {
    value = JSON.parse(event.detail);
  } catch {
    return [];
  }
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return [];
  const record = value as Record<string, unknown>;
  const trace = record.route_trace;
  if (trace && typeof trace === 'object' && !Array.isArray(trace) && Array.isArray((trace as Record<string, unknown>).attempts)) {
    return ((trace as Record<string, unknown>).attempts as unknown[]).flatMap((raw) => {
      if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return [];
      const attempt = raw as Record<string, unknown>;
      return typeof attempt.provider_id === 'string' && (attempt.outcome === 'succeeded' || attempt.outcome === 'failed')
        ? [{ provider: attempt.provider_id, success: attempt.outcome === 'succeeded' }]
        : [];
    });
  }
  if (typeof record.provider !== 'string' || record.provider.trim() === '') return [];
  if (typeof record.success === 'boolean') return [{ provider: record.provider, success: record.success }];
  if (typeof record.status === 'number' && Number.isInteger(record.status) && record.status >= 100 && record.status <= 599) {
    return [{ provider: record.provider, success: record.status >= 200 && record.status < 300 }];
  }
  return [];
}

export function summarizeProviderAvailability(
  providerIds: readonly string[],
  events: readonly ActivityEvent[],
  windowSize = 15,
): Record<string, ProviderAvailability> {
  const result: Record<string, ProviderAvailability> = {};
  for (const providerId of providerIds) {
    const samples = events
      .filter((event) => event.kind === 'request')
      .flatMap(readProviderSamples)
      .filter((sample) => sample.provider === providerId)
      .slice(0, windowSize)
      .reverse()
      .map((sample) => sample.success ? 'success' as const : 'failure' as const);
    const successes = samples.filter((sample) => sample === 'success').length;
    result[providerId] = {
      samples,
      successes,
      rate: samples.length ? (successes / samples.length) * 100 : null,
    };
  }
  return result;
}
