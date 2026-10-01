export const ROUTE_ACTIVITY_WINDOW_MS = 5 * 60 * 1000;
export const ROUTE_ACTIVITY_PULSE_MS = 8 * 1000;
export const ROUTE_ACTIVITY_GLOW_MS = 3 * 1000;
export const ROUTE_ACTIVITY_TIMEOUT_MS = 120 * 1000;
export const ROUTE_ACTIVITY_RESULT_LIMIT = 100;

export interface ActivityRoute {
  left: string;
  match: 'exact' | 'prefix';
  right: string;
  upstream_model?: string;
}

interface OutcomeSample {
  at: number;
  success: boolean;
}

export interface EdgeActivityState {
  active: Record<string, number>;
  resolved: Record<string, number>;
  pulses: number[];
  glows: number[];
  outcomes: OutcomeSample[];
}

export type RouteActivityState = Record<string, EdgeActivityState>;

export interface EdgeActivityVisual {
  activeCount: number;
  lineLevel: number;
  glowLevel: number;
  hue: number;
  saturation: number;
  outcomeCount: number;
  successRate: number | null;
}

interface RouteActivityDetail {
  phase?: unknown;
  activity_id?: unknown;
  requested_model?: unknown;
  provider_id?: unknown;
  upstream_model?: unknown;
  hops?: unknown;
  route_edges?: unknown;
  attempt?: unknown;
}

function routeIdentity(route: ActivityRoute): string {
  return JSON.stringify([route.left, route.match, route.right, route.upstream_model ?? null]);
}

function parseRouteEdges(routes: readonly ActivityRoute[], detail: RouteActivityDetail): string[] {
  if (Array.isArray(detail.route_edges)) {
    const identities = new Set(routes.map(routeIdentity));
    return detail.route_edges.filter((item): item is ActivityRoute => {
      if (!item || typeof item !== 'object') return false;
      const value = item as Partial<ActivityRoute>;
      return typeof value.left === 'string'
        && (value.match === 'exact' || value.match === 'prefix')
        && typeof value.right === 'string';
    }).map(routeIdentity).filter((identity) => identities.has(identity));
  }

  if (typeof detail.requested_model !== 'string' || !Array.isArray(detail.hops)) return [];
  const hops = detail.hops.filter((hop): hop is string => typeof hop === 'string');
  const path = [detail.requested_model, ...hops];
  const selectedModel = typeof detail.upstream_model === 'string' ? detail.upstream_model : null;
  const selectedProvider = typeof detail.provider_id === 'string' ? detail.provider_id : null;
  const identities: string[] = [];
  for (let index = 0; index < path.length - 1; index += 1) {
    const left = path[index];
    const right = path[index + 1];
    const terminal = right === selectedProvider;
    const candidates = routes.filter((route) => {
      const sourceMatches = route.left === left
        || (index === 0 && route.match === 'prefix' && detail.requested_model!.toString().startsWith(route.left.replace(/\*$/, '')));
      return sourceMatches && route.right === right
        && (!terminal || route.upstream_model == null || route.upstream_model === selectedModel);
    });
    identities.push(...candidates.map(routeIdentity));
  }
  return [...new Set(identities)];
}

function blankEdge(): EdgeActivityState {
  return { active: {}, resolved: {}, pulses: [], glows: [], outcomes: [] };
}

function pruneEdge(state: EdgeActivityState, now: number): EdgeActivityState {
  const active = { ...state.active };
  const resolved = Object.fromEntries(Object.entries(state.resolved).filter(([, at]) => now - at <= ROUTE_ACTIVITY_WINDOW_MS));
  const outcomes = state.outcomes.filter((sample) => now - sample.at <= ROUTE_ACTIVITY_WINDOW_MS).slice(-ROUTE_ACTIVITY_RESULT_LIMIT);
  const glows = state.glows.filter((at) => now - at < ROUTE_ACTIVITY_GLOW_MS);
  const pulses = state.pulses.filter((at) => now - at < ROUTE_ACTIVITY_PULSE_MS);

  for (const [id, startedAt] of Object.entries(active)) {
    if (now - startedAt < ROUTE_ACTIVITY_TIMEOUT_MS) continue;
    delete active[id];
    resolved[id] = now;
    outcomes.push({ at: now, success: false });
    glows.push(now);
  }

  return { active, resolved, outcomes: outcomes.slice(-ROUTE_ACTIVITY_RESULT_LIMIT), glows, pulses };
}

/** Apply real per-candidate lifecycle events to the route edges they actually traversed. */
export function applyRouteActivityEvent(
  current: RouteActivityState,
  routes: readonly ActivityRoute[],
  event: { kind: string; id?: number | string; detail: string },
  now: number,
): RouteActivityState {
  if (event.kind !== 'route_activity') return current;
  let detail: RouteActivityDetail;
  try {
    detail = JSON.parse(event.detail) as RouteActivityDetail;
  } catch {
    return current;
  }
  const phase = typeof detail.phase === 'string' ? detail.phase : '';
  if (!['started', 'retrying', 'finished_success', 'finished_failure', 'cancelled'].includes(phase)) return current;
  const activityId = detail.activity_id == null ? null : String(detail.activity_id);
  const identities = parseRouteEdges(routes, detail);
  if (!identities.length) return current;

  const next = { ...current };
  const activityKey = activityId ?? `event-${event.id ?? now}`;
  for (const identity of identities) {
    const previous = pruneEdge(current[identity] ?? blankEdge(), now);
    const state = { ...previous, active: { ...previous.active }, resolved: { ...previous.resolved }, pulses: [...previous.pulses], glows: [...previous.glows], outcomes: [...previous.outcomes] };
    if (phase === 'started') {
      state.active[activityKey] = now;
      state.pulses.push(now);
    } else if (phase === 'retrying') {
      state.pulses.push(now);
    } else if (phase === 'finished_success' || phase === 'finished_failure') {
      if (state.resolved[activityKey] === undefined) {
        delete state.active[activityKey];
        state.resolved[activityKey] = now;
        state.outcomes.push({ at: now, success: phase === 'finished_success' });
        state.glows.push(now);
      }
    } else {
      if (state.resolved[activityKey] === undefined) {
        delete state.active[activityKey];
        state.resolved[activityKey] = now;
      }
    }
    next[identity] = pruneEdge(state, now);
  }
  return next;
}

export function retainRouteActivities(state: RouteActivityState, routes: readonly ActivityRoute[]): RouteActivityState {
  const allowed = new Set(routes.map(routeIdentity));
  return Object.fromEntries(Object.entries(state).filter(([identity]) => allowed.has(identity)));
}

export function advanceRouteActivities(state: RouteActivityState, now: number): RouteActivityState {
  return Object.fromEntries(Object.entries(state)
    .map(([identity, value]) => [identity, pruneEdge(value, now)] as const)
    .filter(([, value]) => Object.keys(value.active).length !== 0 || value.pulses.length !== 0 || value.glows.length !== 0 || value.outcomes.length !== 0));
}

export function nextRouteActivityUpdateDelay(state: RouteActivityState, now: number): number | null {
  let next = Number.POSITIVE_INFINITY;
  for (const value of Object.values(state)) {
    if (value.pulses.some((at) => now - at < ROUTE_ACTIVITY_PULSE_MS)
      || value.glows.some((at) => now - at < ROUTE_ACTIVITY_GLOW_MS)) return 100;
    for (const at of Object.values(value.active)) next = Math.min(next, Math.max(1, at + ROUTE_ACTIVITY_TIMEOUT_MS - now));
    for (const { at } of value.outcomes) next = Math.min(next, Math.max(1, at + ROUTE_ACTIVITY_WINDOW_MS - now));
  }
  return Number.isFinite(next) ? Math.min(next, 30_000) : null;
}

export function edgeActivityVisual(state: EdgeActivityState | undefined, now: number): EdgeActivityVisual {
  if (!state) return { activeCount: 0, lineLevel: 0, glowLevel: 0, hue: 0, saturation: 0, outcomeCount: 0, successRate: null };
  const activeCount = Object.values(state.active).filter((at) => now - at < ROUTE_ACTIVITY_TIMEOUT_MS).length;
  const recentPulses = state.pulses.filter((at) => now - at < ROUTE_ACTIVITY_PULSE_MS);
  const pulseIntensity = recentPulses.reduce((sum, at) => sum + Math.exp(-(now - at) / 1800), 0);
  const pulseLevel = 1 - Math.exp(-pulseIntensity / 1.35);
  const inFlightLevel = activeCount > 0 ? 0.68 + 0.32 * (1 - Math.exp(-activeCount / 1.5)) : 0;
  const lineLevel = Math.max(pulseLevel, inFlightLevel);
  const glowLevel = Math.max(0, ...state.glows.filter((at) => now - at < ROUTE_ACTIVITY_GLOW_MS).map((at) => Math.exp(-(now - at) / 1200)));
  const outcomes = state.outcomes.filter((sample) => now - sample.at <= ROUTE_ACTIVITY_WINDOW_MS).slice(-ROUTE_ACTIVITY_RESULT_LIMIT);
  const successRate = outcomes.length ? outcomes.filter((sample) => sample.success).length / outcomes.length : null;
  return {
    activeCount,
    lineLevel,
    glowLevel,
    hue: successRate === null ? 0 : 120 * successRate,
    saturation: outcomes.length ? Math.min(86, 24 + outcomes.length * 0.6) : 0,
    outcomeCount: outcomes.length,
    successRate,
  };
}
