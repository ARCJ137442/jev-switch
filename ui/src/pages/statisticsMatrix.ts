export interface MatrixEvent {
  timestamp: number;
  kind: string;
  detail: string;
}

export interface HealthCell {
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

function utcDay(timestamp: number): string {
  return new Date(timestamp).toISOString().slice(0, 10);
}

function dateKeys(endMs: number, dayCount: number): string[] {
  const end = new Date(endMs);
  end.setUTCHours(0, 0, 0, 0);
  return Array.from({ length: dayCount }, (_, index) => {
    const date = new Date(end.getTime() - (dayCount - index - 1) * 86_400_000);
    return utcDay(date.getTime());
  });
}

export function buildHealthRows(
  events: readonly MatrixEvent[],
  dimension: 'entry' | 'provider',
  endMs: number,
  dayCount = 30,
): HealthRow[] {
  const days = dateKeys(endMs, dayCount);
  const allowedDays = new Set(days);
  const groups = new Map<string, Map<string, { success: number; failure: number }>>();
  for (const event of events) {
    const detail = parseDetail(event);
    if (!detail) continue;
    const id = dimension === 'entry' ? detail.endpoint_id : detail.provider;
    const day = utcDay(event.timestamp);
    if (typeof id !== 'string' || !id || !allowedDays.has(day)) continue;
    let byDay = groups.get(id);
    if (!byDay) groups.set(id, byDay = new Map());
    let cell = byDay.get(day);
    if (!cell) byDay.set(day, cell = { success: 0, failure: 0 });
    if (detail.success === true) cell.success += 1;
    else if (detail.success === false) cell.failure += 1;
  }

  return [...groups.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([id, byDay]) => ({
    id,
    label: id,
    cells: days.map((day) => {
      const result = byDay.get(day);
      const success = result?.success ?? 0;
      const failure = result?.failure ?? 0;
      const count = success + failure;
      return { day, count, success, failure, rate: count ? success / count : null };
    }),
  }));
}
