import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';
const { buildDag, buildDagLayout, corridorKey, nodePorts, nearestPort, routeIdentity, routeDocumentSignature, wheelPanDelta, wirePath } = await loadTs('../src/components/routing/dag.ts');
const { DocumentHistory } = await loadTs('../src/components/routing/documentHistory.ts');
const route = (left, right, model) => ({ left, right, upstream_model: model, match: 'exact', priority: 10, sticky: 'session', on_error: 'next' });

test('wheel gestures pan the routing canvas vertically and with Shift horizontally', () => {
  assert.deepEqual(wheelPanDelta(0, 120, false), { x: 0, y: -120 });
  assert.deepEqual(wheelPanDelta(0, 120, true), { x: -120, y: 0 });
  assert.deepEqual(wheelPanDelta(18, 120, true), { x: -18, y: 0 }, 'native horizontal delta wins when present');
  assert.deepEqual(wheelPanDelta(-24, 40, false), { x: 24, y: -40 });
});

test('multi-hop branches have left-to-right columns and separate same-address account cards', () => {
  const routes = [route('public', 'fallback'), route('fallback', 'personal', 'M1'), route('fallback', 'personal', 'M2'), route('public', 'team', 'M1')];
  const nodes = buildDag(routes, [{ id: 'public', enabled: true }], [
    { id: 'personal', base: 'https://same.example', account: 'Personal', enabled: true, models: ['M1', 'M2'] },
    { id: 'team', base: 'https://same.example', account: 'Team', enabled: true, models: ['M1'] },
  ]);
  const byId = Object.fromEntries(nodes.map(n => [n.id, n]));
  for (const edge of routes) assert(byId[edge.left].x + byId[edge.left].width < byId[edge.right].x);
  assert.equal(nodes.filter(n => n.kind === 'provider').length, 2);
  assert.equal(byId.fallback.kind, 'alias');
  assert.deepEqual(byId.personal.models, ['M1', 'M2', null]);
  assert.equal(byId.personal.detail, 'Personal');
  assert.equal(byId.team.detail, 'Team');
  assert.notEqual(routeIdentity(routes[1]), routeIdentity(routes[2]), 'different model ports are independent routes');
});

test('snap resolves correct model port inside radius without requiring a DOM hit', () => {
  const nodes = buildDag([route('entry', 'provider', 'M2')], [{ id: 'entry', enabled: true }], [{ id: 'provider', enabled: true, models: ['M1', 'M2'] }]);
  const ports = nodes.flatMap(nodePorts);
  const target = ports.find(p => p.id === 'provider' && p.model === 'M2');
  assert.equal(nearestPort(ports, { x: target.x - 35, y: target.y }, 'input', 'entry'), target);
  assert.equal(nearestPort(ports, { x: target.x - 41, y: target.y }, 'input', 'entry'), null);
  assert.equal(nearestPort(ports, target, 'input', 'provider'), null, 'self connection excluded');
  assert.equal(nearestPort(ports, target, 'output', 'entry'), null, 'direction is enforced');
});

test('layout is independent of route row order, and saved account positions survive reconnects', () => {
  const entries = [{ id: 'entry', enabled: true }];
  const providers = [{ id: 'personal', enabled: true, models: ['M1'] }, { id: 'team', enabled: true, models: ['M1'] }];
  const routes = [route('entry', 'alias'), route('alias', 'personal', 'M1'), route('entry', 'team', 'M1')];
  const positions = rows => Object.fromEntries(buildDag(rows, entries, providers).map(n => [n.id, [n.x, n.y]]));
  assert.deepEqual(positions(routes), positions([...routes].reverse()));
  const saved = { personal: { x: 900, y: 120 }, team: { x: 900, y: 420 } };
  const reconnected = buildDag([routes[0], { ...routes[1], right: 'team' }, routes[2]], entries, providers, saved);
  for (const node of reconnected.filter(node => node.kind === 'provider')) assert.deepEqual({ x: node.x, y: node.y }, saved[node.id]);
});

test('Sugiyama layout separates variable-height cards and leaves room for long-hop edges', () => {
  const routes = [route('entry', 'short'), route('entry', 'long'), route('short', 'middle'), route('middle', 'account', 'M4'), route('long', 'account', 'M1')];
  const providers = [{ id: 'account', enabled: true, models: ['M1', 'M2', 'M3', 'M4'] }];
  const nodes = buildDag(routes, [{ id: 'entry', enabled: true }], providers);
  const byId = Object.fromEntries(nodes.map(node => [node.id, node]));
  for (const edge of routes) assert(byId[edge.left].x + byId[edge.left].width < byId[edge.right].x);
  for (let i = 0; i < nodes.length; i++) for (let j = i + 1; j < nodes.length; j++) {
    const a = nodes[i], b = nodes[j];
    assert(a.x + a.width <= b.x || b.x + b.width <= a.x || a.y + a.height <= b.y || b.y + b.height <= a.y,
      `${a.id} and ${b.id} must not overlap`);
  }
  assert(byId.account.height > byId.entry.height);
});

test('long-hop wire uses Dagre corridor above an intervening card', () => {
  const routes = [route('entry', 'middle'), route('middle', 'provider'), route('entry', 'provider')];
  const layout = buildDagLayout(routes, [{ id: 'entry', enabled: true }], [{ id: 'provider', enabled: true, models: ['M1', 'M2', 'M3'] }]);
  const middle = layout.nodes.find(node => node.id === 'middle');
  const corridor = layout.corridors.get(corridorKey('entry', 'provider'));
  assert(corridor?.length > 0, 'a skipped rank must provide a routing corridor');
  assert(corridor.every(point => point.y < middle.y || point.y > middle.y + middle.height), 'long-hop corridor avoids the intervening card');
  const from = nodePorts(layout.nodes.find(node => node.id === 'entry')).find(port => port.direction === 'output');
  const to = nodePorts(layout.nodes.find(node => node.id === 'provider')).find(port => port.model === null);
  assert.notEqual(wirePath(from, to, corridor), wirePath(from, to), 'the rendered wire must use the corridor');
});

test('history restores every edge attribute and card position after reconnect/delete', () => {
  const initial = { routes: [route('entry', 'personal', 'M1')], positions: { entry: { x: 60, y: 80 } } };
  const history = new DocumentHistory(initial);
  history.push({ routes: [route('entry', 'team', 'M2')], positions: { entry: { x: 180, y: 180 } } });
  history.push({ routes: [], positions: {} });
  assert.equal(history.undo().routes[0].right, 'team');
  assert.deepEqual(history.undo(), initial);
  assert.equal(history.redo().routes[0].upstream_model, 'M2');
  assert.equal(history.current.routes[0].sticky, 'session');
  history.push(initial);
  assert.equal(history.canRedo, false, 'new edit discards only the redo branch');
  initial.routes[0].priority = 999;
  assert.equal(history.current.routes[0].priority, 10, 'snapshots do not alias caller data');
});

test('empty entries stay editable and saved card positions survive layout reconstruction', () => {
  const [node] = buildDag([], [{ id: 'empty-entry', enabled: false }], [], { 'empty-entry': { x: 76, y: 98 } });
  assert.equal(node.kind, 'entry'); assert.equal(node.enabled, false);
  assert.deepEqual({ x: node.x, y: node.y }, { x: 76, y: 98 });
  assert.equal(nodePorts(node).filter(p => p.direction === 'output').length, 1);
});

test('save acknowledgement tolerates row order but detects the observed missing intermediate edge', () => {
  const submitted = [route('public', 'fallback'), route('fallback', 'personal', 'M2'), route('public', 'team', 'M1')];
  assert.equal(routeDocumentSignature(submitted), routeDocumentSignature([...submitted].reverse()));
  assert.notEqual(routeDocumentSignature(submitted), routeDocumentSignature(submitted.slice(1)));
  const lostProperty = submitted.map(r => ({ ...r, sticky: 'none' }));
  assert.notEqual(routeDocumentSignature(submitted), routeDocumentSignature(lostProperty));
});
