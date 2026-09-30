export type ProviderHealth = {
  state: 'reachable' | 'unreachable' | 'disabled' | 'unknown';
  ms: number | null;
};

export type ProbeTarget = { id: string; enabled: boolean };
export type ProbeResult = { ok: boolean; latency_ms: number };

export async function probeProvidersConcurrently(
  providers: readonly ProbeTarget[],
  probe: (id: string) => Promise<ProbeResult>,
  concurrency = 4,
): Promise<Record<string, ProviderHealth>> {
  if (!Number.isInteger(concurrency) || concurrency < 1) {
    throw new RangeError('Probe concurrency must be a positive integer');
  }

  const health: Record<string, ProviderHealth> = {};
  const pending: ProbeTarget[] = [];
  for (const provider of providers) {
    if (provider.enabled) pending.push(provider);
    else health[provider.id] = { state: 'disabled', ms: null };
  }

  let cursor = 0;
  const worker = async () => {
    while (cursor < pending.length) {
      const provider = pending[cursor++];
      try {
        const result = await probe(provider.id);
        health[provider.id] = {
          state: result.ok ? 'reachable' : 'unreachable',
          ms: result.latency_ms,
        };
      } catch {
        health[provider.id] = { state: 'unknown', ms: null };
      }
    }
  };

  await Promise.all(
    Array.from({ length: Math.min(concurrency, pending.length) }, () => worker()),
  );
  return health;
}
