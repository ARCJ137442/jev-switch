import dagre from '@dagrejs/dagre';
import type { Route } from '../../api/admin';

export interface DagProvider {
  id: string; enabled: boolean; name?: string | null; account?: string | null;
  base?: string; api_key_masked?: string | null; models?: string[];
}
export interface DagEntry { id: string; enabled: boolean; strategy?: string }
export interface Point { x: number; y: number }
export interface DagNode extends Point {
  id: string; kind: 'entry' | 'alias' | 'provider'; width: number; height: number;
  enabled: boolean; label: string; detail?: string; models: Array<string | null>; strategy?: string; inCount: number; outCount: number;
}
export interface DagPort extends Point { id: string; direction: 'input' | 'output'; model?: string | null }
export interface DagLayout { nodes: DagNode[]; corridors: Map<string, Point[]> }

export function corridorKey(left: string, right: string): string {
  return JSON.stringify([left, right]);
}

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

/** Dagre inserts virtual nodes for long edges and respects each card's dimensions. */
export function buildDagLayout(routes: Route[], entries: DagEntry[], providers: DagProvider[], positions: Record<string, Point> = {}): DagLayout {
  const providerIds = new Set(providers.map(p => p.id));
  const entryIds = new Set(entries.map(e => e.id));
  // Stable IDs and sorted edges prevent storage row order from changing layout.
  const aliases = [...new Set(routes.flatMap(r => [r.left, r.right]))].filter(id => !providerIds.has(id) && !entryIds.has(id)).sort();
  const allIds = [...new Set([...entryIds, ...aliases, ...providerIds])].sort();
  const metadata = new Map<string, Omit<DagNode, 'x' | 'y'>>();
  for (const id of allIds) {
    const provider = providers.find(p => p.id === id);
    const entry = entries.find(e => e.id === id);
    const kind = provider ? 'provider' : entry ? 'entry' : 'alias';
    const routedModels = routes.filter(r => r.right === id).map(r => r.upstream_model ?? null);
    const models: Array<string | null> = provider ? [...new Set([...(provider.models ?? []), ...routedModels, null])] : [];
    const height = provider ? 98 + models.length * 30 : 90;
    const label = provider?.name || id;
    const detail = provider?.account || provider?.base;
    const width = Math.min(360, Math.max(250, Math.max(label.length, detail?.length ?? 0) * 7.5 + 36));
    const inCount = routes.filter(route => route.right === id).length;
    const outCount = routes.filter(route => route.left === id).length;
    metadata.set(id, {
      id, kind, label, detail,
      enabled: provider?.enabled ?? entry?.enabled ?? true, strategy: entry?.strategy,
      width, height, models, inCount, outCount,
    });
  }

  const graph = new dagre.graphlib.Graph();
  graph.setGraph({ rankdir: 'LR', nodesep: 38, ranksep: 96, marginx: 30, marginy: 56 });
  graph.setDefaultEdgeLabel(() => ({}));
  for (const id of allIds) {
    const { width, height } = metadata.get(id)!;
    graph.setNode(id, { width, height });
  }
  const uniqueEdges = new Set<string>();
  for (const route of [...routes].sort((a, b) => routeIdentity(a).localeCompare(routeIdentity(b)))) {
    if (!metadata.has(route.left) || !metadata.has(route.right) || route.left === route.right) continue;
    const key = corridorKey(route.left, route.right);
    if (uniqueEdges.has(key)) continue;
    uniqueEdges.add(key);
    graph.setEdge(route.left, route.right);
  }
  dagre.layout(graph);
  // Provider accounts share the terminal rank, but retain Dagre's crossing-reduced order.
  const providerColumn = Math.max(30,
    ...allIds.filter(id => !providerIds.has(id)).map(id => {
    const item = graph.node(id);
    return item.x + metadata.get(id)!.width / 2 + 96;
    }),
    ...allIds.filter(id => providerIds.has(id)).map(id => graph.node(id).x - metadata.get(id)!.width / 2));
  const providerOrder = [...providerIds].sort((a, b) => graph.node(a).y - graph.node(b).y || a.localeCompare(b));
  let providerBottom = 56;
  const providerYs = new Map<string, number>();
  for (const id of providerOrder) {
    const node = metadata.get(id)!;
    const y = Math.max(56, graph.node(id).y - node.height / 2, providerBottom);
    providerYs.set(id, y);
    providerBottom = y + node.height + 38;
  }
  const nodes = allIds.map(id => {
    const layout = graph.node(id);
    const node = metadata.get(id)!;
    return {
      ...node,
      x: positions[id]?.x ?? (providerIds.has(id) ? providerColumn : layout.x - node.width / 2),
      y: positions[id]?.y ?? (providerIds.has(id) ? providerYs.get(id)! : layout.y - node.height / 2),
    };
  });
  const corridors = new Map<string, Point[]>();
  for (const edge of graph.edges()) {
    const points = graph.edge(edge)?.points;
    if (Array.isArray(points) && points.length > 2) {
      corridors.set(corridorKey(edge.v, edge.w), points.slice(1, -1).map(point => ({ x: point.x, y: point.y })));
    }
  }
  return { nodes, corridors };
}

export function buildDag(routes: Route[], entries: DagEntry[], providers: DagProvider[], positions: Record<string, Point> = {}): DagNode[] {
  return buildDagLayout(routes, entries, providers, positions).nodes;
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

export function wirePath(from: Point, to: Point, corridor: readonly Point[] = []): string {
  if (corridor.length) {
    const points = [from, ...corridor.filter(point => point.x > from.x && point.x < to.x), to];
    if (points.length > 2) return points.slice(1).reduce((path, end, index) => {
      const start = points[index];
      const bend = Math.max(0, (end.x - start.x) * .42);
      return `${path} C ${start.x + bend} ${start.y}, ${end.x - bend} ${end.y}, ${end.x} ${end.y}`;
    }, `M ${from.x} ${from.y}`);
  }
  const bend = Math.max(70, Math.abs(to.x - from.x) * .45);
  return `M ${from.x} ${from.y} C ${from.x + bend} ${from.y}, ${to.x - bend} ${to.y}, ${to.x} ${to.y}`;
}
