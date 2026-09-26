import type { Route } from '../../api/admin';

export interface DagProvider {
  id: string; enabled: boolean; name?: string | null; account?: string | null;
  base?: string; api_key_masked?: string | null; models?: string[];
}
export interface DagEntry { id: string; enabled: boolean; strategy?: string }
export interface Point { x: number; y: number }
export interface DagNode extends Point {
  id: string; kind: 'entry' | 'alias' | 'provider'; width: number; height: number;
  enabled: boolean; label: string; detail?: string; models: Array<string | null>; strategy?: string;
}
export interface DagPort extends Point { id: string; direction: 'input' | 'output'; model?: string | null }

export function routeIdentity(route: Route): string {
  return JSON.stringify([route.left, route.match, route.right, route.upstream_model ?? null]);
}

/** Server storage may reorder rows, but it must not lose routes or change attributes. */
export function routeDocumentSignature(routes: Route[]): string {
  return JSON.stringify(routes.map(route => JSON.stringify([
    route.left, route.match, route.right, route.upstream_model ?? null,
    route.priority, route.sticky ?? 'none', route.on_error ?? 'next',
  ])).sort());
}

/** Stable topological columns; provider cards are terminals, never duplicate alias boxes. */
export function buildDag(routes: Route[], entries: DagEntry[], providers: DagProvider[], positions: Record<string, Point> = {}): DagNode[] {
  const providerIds = new Set(providers.map(p => p.id));
  const entryIds = new Set(entries.map(e => e.id));
  // Reconnecting a wire or reordering stored rows must not swap account cards.
  const aliases = [...new Set(routes.flatMap(r => [r.left, r.right]))].filter(id => !providerIds.has(id) && !entryIds.has(id)).sort();
  const allIds = [...new Set([...entryIds, ...aliases, ...providerIds])];
  const levels = new Map(allIds.map(id => [id, 0]));
  const incoming = new Map(allIds.map(id => [id, 0]));
  const outgoing = new Map<string, string[]>();
  for (const route of routes) {
    outgoing.set(route.left, [...(outgoing.get(route.left) ?? []), route.right]);
    incoming.set(route.right, (incoming.get(route.right) ?? 0) + 1);
  }
  const queue = allIds.filter(id => incoming.get(id) === 0);
  for (let i = 0; i < queue.length; i++) {
    const id = queue[i];
    for (const right of outgoing.get(id) ?? []) {
      levels.set(right, Math.max(levels.get(right) ?? 0, (levels.get(id) ?? 0) + 1));
      incoming.set(right, (incoming.get(right) ?? 1) - 1);
      if (incoming.get(right) === 0) queue.push(right);
    }
  }
  const lastColumn = Math.max(1, ...allIds.filter(id => !providerIds.has(id)).map(id => (levels.get(id) ?? 0) + 1));
  const offsets = new Map<number, number>();
  return allIds.map(id => {
    const provider = providers.find(p => p.id === id);
    const entry = entries.find(e => e.id === id);
    const kind = provider ? 'provider' : entry ? 'entry' : 'alias';
    const column = provider ? lastColumn : levels.get(id) ?? 0;
    const routedModels = routes.filter(r => r.right === id).map(r => r.upstream_model ?? null);
    const models: Array<string | null> = provider ? [...new Set([...(provider.models ?? []), ...routedModels, null])] : [];
    const height = provider ? 98 + models.length * 30 : 90;
    const y = offsets.get(column) ?? 56;
    offsets.set(column, y + height + 24);
    return {
      id, kind, label: provider?.name || id, detail: provider?.account || provider?.base,
      enabled: provider?.enabled ?? entry?.enabled ?? true, strategy: entry?.strategy,
      x: positions[id]?.x ?? 30 + column * 330, y: positions[id]?.y ?? y,
      width: 250, height, models,
    };
  });
}

export function nodePorts(node: DagNode): DagPort[] {
  if (node.kind === 'provider') return node.models.map((model, i) => ({
    id: node.id, direction: 'input', model, x: node.x, y: node.y + 105 + i * 30,
  }));
  return [
    { id: node.id, direction: 'input', x: node.x, y: node.y + 45 },
    { id: node.id, direction: 'output', x: node.x + node.width, y: node.y + 45 },
  ];
}

export function nearestPort(ports: DagPort[], point: Point, direction: DagPort['direction'], excludeId: string, radius = 40): DagPort | null {
  let nearest: DagPort | null = null;
  let distance = radius;
  for (const port of ports) {
    if (port.direction !== direction || port.id === excludeId) continue;
    const candidate = Math.hypot(port.x - point.x, port.y - point.y);
    if (candidate <= distance) { distance = candidate; nearest = port; }
  }
  return nearest;
}

export function wirePath(from: Point, to: Point): string {
  const bend = Math.max(70, Math.abs(to.x - from.x) * .45);
  return `M ${from.x} ${from.y} C ${from.x + bend} ${from.y}, ${to.x - bend} ${to.y}, ${to.x} ${to.y}`;
}
