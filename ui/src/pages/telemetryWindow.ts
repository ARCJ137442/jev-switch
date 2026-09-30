export interface TrafficSample {
  sample_at_ms: number;
  ingress_bps: number;
  egress_bps: number;
}

export interface TrafficPoint extends TrafficSample {}

export const TRAFFIC_WINDOW_MS = 10 * 60 * 1000;
export const TRAFFIC_WINDOW_POINTS = 61;

export function buildTrafficWindow(
  samples: readonly TrafficSample[],
  nowMs: number,
  pointCount = TRAFFIC_WINDOW_POINTS,
): TrafficPoint[] {
  if (pointCount < 2) return [];
  const startMs = nowMs - TRAFFIC_WINDOW_MS;
  const inWindow = samples
    .filter((sample) => Number.isFinite(sample.sample_at_ms) && sample.sample_at_ms >= startMs && sample.sample_at_ms <= nowMs)
    .slice()
    .sort((left, right) => left.sample_at_ms - right.sample_at_ms);
  let sampleIndex = 0;
  let latest: TrafficSample | undefined;
  return Array.from({ length: pointCount }, (_, index) => {
    const sampleAt = startMs + (TRAFFIC_WINDOW_MS * index) / (pointCount - 1);
    while (sampleIndex < inWindow.length && inWindow[sampleIndex].sample_at_ms <= sampleAt) {
      latest = inWindow[sampleIndex++];
    }
    return {
      sample_at_ms: sampleAt,
      ingress_bps: latest?.ingress_bps ?? 0,
      egress_bps: latest?.egress_bps ?? 0,
    };
  });
}
