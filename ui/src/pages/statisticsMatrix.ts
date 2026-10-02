export interface MatrixEvent {
  timestamp: number;
  kind: string;
  detail: string;
}

export type HeatmapGranularity = 'day' | 'hour' | 'minute' | 'second';

export interface HealthCell {
  /** Kept as `day` for the existing UI contract; it is a bucket label at finer scales. */
  day: string;
  count: number;
  success: number;
  failure: number;
  rate: number | null;
}

export interface HealthRow {
  id: string;
  label: string;
  cells: HealthCell[];
}

export interface AdaptiveHealthMatrix {
  rows: HealthRow[];
  granularity: HeatmapGranularity;
  buckets: string[];
}

const GRANULARITY_MS: Record<HeatmapGranularity, number> = {
  second: 1_000,
  minute: 60_000,
  hour: 3_600_000,
  day: 86_400_000,
};

function parseDetail(event: MatrixEvent): Record<string, unknown> | null {
  if (event.kind !== 'request') return null;
  try {
    const detail: unknown = JSON.parse(event.detail);
    return detail !== null && typeof detail === 'object' && !Array.isArray(detail)
      ? detail as Record<string, unknown>
      : null;
  } catch {
    return null;
  }
}

function bucketStart(timestamp: number, granularity: HeatmapGranularity): number {
  return Math.floor(timestamp / GRANULARITY_MS[granularity]) * GRANULARITY_MS[granularity];
}

export function formatHeatmapBucket(timestamp: number, granularity: HeatmapGranularity): string {
  const date = new Date(bucketStart(timestamp, granularity));
  if (granularity === 'day') return date.toISOString().slice(0, 10);
  if (granularity === 'hour') return `${date.toISOString().slice(0, 13)}:00Z`;
  if (granularity === 'minute') return `${date.toISOString().slice(0, 16)}Z`;
  return `${date.toISOString().slice(0, 19)}Z`;
}

function bucketKeys(startMs: number, endMs: number, granularity: HeatmapGranularity): string[] {
  const size = GRANULARITY_MS[granularity];
  const start = bucketStart(Math.min(startMs, endMs), granularity);
  const end = bucketStart(Math.max(startMs, endMs), granularity);
  const count = Math.max(1, Math.floor((end - start) / size) + 1);
  return Array.from({ length: count }, (_, index) => formatHeatmapBucket(start + index * size, granularity));
}

/** Pick the finest time unit that fits square cells in the available width. */
export function selectHeatmapGranularity(
  startMs: number,
  endMs: number,
  availableWidth: number,
  labelWidth = 112,
  minCellSize = 7,
): HeatmapGranularity {
  const range = Math.max(0, Math.abs(endMs - startMs));
  const capacity = Math.max(1, Math.floor(Math.max(0, availableWidth - labelWidth) / minCellSize));
  for (const granularity of ['second', 'minute', 'hour', 'day'] as const) {
    if (Math.floor(range / GRANULARITY_MS[granularity]) + 1 <= capacity) return granularity;
  }
  return 'day';
}

function buildRowsForBuckets(
  events: readonly MatrixEvent[],
  dimension: 'entry' | 'provider',
  buckets: readonly string[],
  granularity: HeatmapGranularity,
  startMs: number,
  endMs: number,
  providerFilter = 'all',
): HealthRow[] {
  const allowed = new Set(buckets);
  const groups = new Map<string, Map<string, { success: number; failure: number }>>();
  for (const event of events) {
    if (event.timestamp < startMs || event.timestamp > endMs) continue;
    const detail = parseDetail(event);
    if (!detail) continue;
    const bucket = formatHeatmapBucket(event.timestamp, granularity);
    if (!allowed.has(bucket)) continue;
    const trace = detail.route_trace;
    const attempts = trace && typeof trace === 'object' && !Array.isArray(trace)
      ? (trace as Record<string, unknown>).attempts : null;
    const samples: Array<{ id: unknown; success: unknown }> = dimension === 'entry'
      ? [{ id: detail.endpoint_id, success: detail.success }]
      : Array.isArray(attempts)
        ? attempts.flatMap((raw) => {
          if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return [];
          const attempt = raw as Record<string, unknown>;
          return attempt.outcome === 'succeeded' || attempt.outcome === 'failed'
            ? [{ id: attempt.provider_id, success: attempt.outcome === 'succeeded' }]
            : [];
        })
        : [{ id: detail.provider, success: detail.success }];
    for (const { id, success } of samples) {
      if (typeof id !== 'string' || !id) continue;
      if (dimension === 'provider' && providerFilter !== 'all' && id !== providerFilter) continue;
      let byBucket = groups.get(id);
      if (!byBucket) groups.set(id, byBucket = new Map());
      let cell = byBucket.get(bucket);
      if (!cell) byBucket.set(bucket, cell = { success: 0, failure: 0 });
      if (success === true) cell.success += 1;
      else if (success === false) cell.failure += 1;
    }
  }

  return [...groups.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([id, byBucket]) => ({
    id,
    label: id,
    cells: buckets.map((bucket) => {
      const result = byBucket.get(bucket);
      const success = result?.success ?? 0;
      const failure = result?.failure ?? 0;
      const count = success + failure;
      return { day: bucket, count, success, failure, rate: count ? success / count : null };
    }),
  }));
}

function dateKeys(endMs: number, dayCount: number): string[] {
  const end = bucketStart(endMs, 'day');
  return Array.from({ length: dayCount }, (_, index) => formatHeatmapBucket(end - (dayCount - index - 1) * GRANULARITY_MS.day, 'day'));
}

/** Existing fixed-day API retained for callers and historical tests. */
export function buildHealthRows(
  events: readonly MatrixEvent[],
  dimension: 'entry' | 'provider',
  endMs: number,
  dayCount = 30,
): HealthRow[] {
  const buckets = dateKeys(endMs, dayCount);
  const startMs = bucketStart(endMs, 'day') - (dayCount - 1) * GRANULARITY_MS.day;
  return buildRowsForBuckets(events, dimension, buckets, 'day', startMs, endMs + GRANULARITY_MS.day - 1);
}

export function buildAdaptiveHealthRows(
  events: readonly MatrixEvent[],
  dimension: 'entry' | 'provider',
  startMs: number,
  endMs: number,
  availableWidth: number,
  providerFilter = 'all',
): AdaptiveHealthMatrix {
  const safeStart = Math.min(startMs, endMs);
  const safeEnd = Math.max(startMs, endMs);
  const granularity = selectHeatmapGranularity(safeStart, safeEnd, availableWidth);
  const buckets = bucketKeys(safeStart, safeEnd, granularity);
  return {
    rows: buildRowsForBuckets(events, dimension, buckets, granularity, safeStart, safeEnd, providerFilter),
    granularity,
    buckets,
  };
}
