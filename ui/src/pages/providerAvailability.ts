import type { ActivityEvent } from '../api/access';

export interface ProviderAvailability {
  samples: Array<'success' | 'failure'>;
  successes: number;
  rate: number | null;
}

function readProviderSample(event: ActivityEvent): { provider: string; success: boolean } | null {
  if (event.kind !== 'request') return null;
  let value: unknown;
  try {
    value = JSON.parse(event.detail);
  } catch {
    return null;
  }
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return null;
  const record = value as Record<string, unknown>;
  if (typeof record.provider !== 'string' || record.provider.trim() === '') return null;
  if (typeof record.success === 'boolean') return { provider: record.provider, success: record.success };
  if (typeof record.status === 'number' && Number.isInteger(record.status) && record.status >= 100 && record.status <= 599) {
    return { provider: record.provider, success: record.status >= 200 && record.status < 300 };
  }
  return null;
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
      .map(readProviderSample)
      .filter((sample): sample is { provider: string; success: boolean } => sample?.provider === providerId)
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
