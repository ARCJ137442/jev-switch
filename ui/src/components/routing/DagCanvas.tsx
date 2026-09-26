import { useId, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react';
import { edgeKey, type Route } from '../../api/admin';
import { useI18n } from '../../i18n';
import { EdgeInspector } from './EdgeInspector';
import { buildDag, nearestPort, nodePorts, routeIdentity, wirePath, type DagEntry, type DagPort, type DagProvider, type Point } from './dag';
import './dag.css';

interface Props {
  routes: Route[]; entries: DagEntry[]; providers: DagProvider[];
  positions: Record<string, Point>; selected: string | null; errors: ReadonlySet<string>;
  onSelect: (id: string | null) => void;
  onCreate: (left: string, right: string, model?: string | null) => void;
  onPatch: (key: string, patch: Partial<Route>) => void;
  onDelete: (key: string) => void;
  onDeleteNode: (id: string) => void;
  onInsertNode: (key: string, id: string) => void;
  onMove: (id: string, position: Point) => void;
  onResetLayout: () => void;
}
type WireDrag = { type: 'wire'; fixed: DagPort; moving: Point; key?: string; end: 'left' | 'right' };
type NodeDrag = { type: 'node'; id: string; offset: Point; point: Point };
type PanDrag = { type: 'pan'; start: Point; scroll: Point };
type Drag = WireDrag | NodeDrag | PanDrag;
const samePort = (a: DagPort | null, b: DagPort) => a?.id === b.id && a.direction === b.direction && a.model === b.model;

/** Shared graph coordinates make snap/reconnect work even when released between DOM elements. */
export function DagCanvas(props: Props) {
  const { t } = useI18n();
  const viewport = useRef<HTMLDivElement>(null);
  const world = useRef<HTMLDivElement>(null);
  const dragRef = useRef<Drag | null>(null);
  const [drag, setDragState] = useState<Drag | null>(null);
  const [zoom, setZoom] = useState(1);
  const [nodeId, setNodeId] = useState('');
  const [context, setContext] = useState<{ x: number; y: number; edge?: string; node?: string } | null>(null);
  const marker = useId().replace(/:/g, '');
  const setDrag = (next: Drag | null) => { dragRef.current = next; setDragState(next); };
  const positions = drag?.type === 'node' ? { ...props.positions, [drag.id]: drag.point } : props.positions;
  const nodes = useMemo(() => buildDag(props.routes, props.entries, props.providers, positions), [props.routes, props.entries, props.providers, positions]);
  const ports = nodes.flatMap(nodePorts);
  const width = Math.max(1000, ...nodes.map(n => n.x + n.width + 80));
  const height = Math.max(460, ...nodes.map(n => n.y + n.height + 80));
  const selectedRoute = props.routes.find(r => routeIdentity(r) === props.selected);
  const snap = drag?.type === 'wire' ? nearestPort(ports, drag.moving, drag.end === 'right' ? 'input' : 'output', drag.fixed.id, 40 / zoom) : null;
  const point = (event: { clientX: number; clientY: number }): Point => {
    const bounds = world.current!.getBoundingClientRect();
    return { x: (event.clientX - bounds.left) / zoom, y: (event.clientY - bounds.top) / zoom };
  };
  const capture = (event: ReactPointerEvent) => { event.preventDefault(); event.stopPropagation(); viewport.current?.setPointerCapture(event.pointerId); };
  const startWire = (port: DagPort, event?: ReactPointerEvent, key?: string, end?: 'left' | 'right') => {
    if (event && event.button !== 0) return;
    if (event) capture(event);
    setContext(null);
    setDrag({ type: 'wire', fixed: port, moving: port, key, end: end ?? (port.direction === 'output' ? 'right' : 'left') });
  };
  const finishWire = (active: WireDrag, target: DagPort) => {
    if (active.key) {
      props.onPatch(active.key, active.end === 'left'
        ? { left: target.id, match: target.id.endsWith('*') ? 'prefix' : 'exact' }
        : { right: target.id, upstream_model: target.model ?? undefined });
    } else if (active.end === 'right') props.onCreate(active.fixed.id, target.id, target.model);
    else props.onCreate(target.id, active.fixed.id, active.fixed.model);
    setDrag(null);
  };
  const portKey = (port: DagPort) => JSON.stringify([port.id, port.direction, port.model]);
  const inputPort = (route: Route) => ports.find(p => p.id === route.right && p.direction === 'input' && (p.model === undefined || (p.model ?? null) === (route.upstream_model ?? null)));
  const outputPort = (route: Route) => ports.find(p => p.id === route.left && p.direction === 'output');

  return <section className="dag-workbench" aria-label={t('entry.graph')}>
    <div className="dag-tools">
      <span className="dag-help">{t('dag.hint')}</span>
      <div className="dag-tool-group">
        <button type="button" onClick={() => setZoom(z => Math.max(.4, Math.round((z - .1) * 10) / 10))} aria-label={t('dag.zoomOut')}>−</button>
        <output>{Math.round(zoom * 100)}%</output>
        <button type="button" onClick={() => setZoom(z => Math.min(1.6, Math.round((z + .1) * 10) / 10))} aria-label={t('dag.zoomIn')}>+</button>
        <button type="button" onClick={() => { props.onResetLayout(); setZoom(1); viewport.current?.scrollTo(0, 0); }}>{t('dag.layout')}</button>
      </div>
    </div>
    {selectedRoute && <div className="dag-tools">
      <label className="dag-insert"><span>{t('dag.nodeId')}</span><input value={nodeId} onChange={e => setNodeId(e.target.value)} spellCheck={false} placeholder="fallback-group" /></label>
      <button type="button" disabled={!nodeId.trim()} onClick={() => { props.onInsertNode(props.selected!, nodeId.trim()); setNodeId(''); }}>{t('dag.insert')}</button>
    </div>}
    <div className="dag-viewport" ref={viewport} tabIndex={0}
      onKeyDown={event => { if (event.key === 'Escape') { setDrag(null); setContext(null); props.onSelect(null); } }}
      onPointerDown={event => {
        if ((event.target as HTMLElement).closest('button,input,select,[data-node],.dag-edge')) return;
        setContext(null); props.onSelect(null);
        if (event.button === 1 || event.button === 0) { capture(event); setDrag({ type: 'pan', start: { x: event.clientX, y: event.clientY }, scroll: { x: viewport.current!.scrollLeft, y: viewport.current!.scrollTop } }); }
      }}
      onPointerMove={event => {
        const active = dragRef.current;
        if (!active) return;
        if (active.type === 'pan') { viewport.current!.scrollLeft = active.scroll.x - event.clientX + active.start.x; viewport.current!.scrollTop = active.scroll.y - event.clientY + active.start.y; return; }
        const pos = point(event);
        if (active.type === 'wire') setDrag({ ...active, moving: pos });
        else setDrag({ ...active, point: { x: Math.max(14, pos.x - active.offset.x), y: Math.max(34, pos.y - active.offset.y) } });
      }}
      onPointerUp={event => {
        const active = dragRef.current;
        if (!active) return;
        if (active.type === 'wire') {
          const target = nearestPort(ports, point(event), active.end === 'right' ? 'input' : 'output', active.fixed.id, 40 / zoom);
          if (target) finishWire(active, target);
        } else if (active.type === 'node') props.onMove(active.id, active.point);
        setDrag(null);
        if (viewport.current?.hasPointerCapture(event.pointerId)) viewport.current.releasePointerCapture(event.pointerId);
      }}
      onPointerCancel={() => setDrag(null)}>
      <div style={{ width: width * zoom, height: height * zoom }}>
        <div ref={world} className="dag-world" style={{ width, height, transform: `scale(${zoom})` }}>
          <svg className="dag-wires" width={width} height={height} aria-label={t('dag.connections')}>
            <defs><marker id={marker} viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="context-stroke" /></marker></defs>
            {props.routes.map((route, index) => {
              const from = outputPort(route); const to = inputPort(route); if (!from || !to) return null;
              const key = routeIdentity(route); const selected = key === props.selected;
              const invalid = props.errors.has(edgeKey(route.left, route.right));
              return <g key={`${key}:${index}`} className={`dag-edge${selected ? ' selected' : ''}${invalid ? ' invalid' : ''}`}
                role="button" tabIndex={0} aria-pressed={selected} aria-label={`${route.left} → ${route.right}${route.upstream_model ? ` / ${route.upstream_model}` : ''}`}
                onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.stopPropagation(); props.onSelect(key); } }}>
                <path className="dag-wire" d={wirePath(from, to)} markerEnd={`url(#${marker})`} />
                <path className="dag-hit" d={wirePath(from, to)} onClick={e => { e.stopPropagation(); props.onSelect(key); }} onContextMenu={e => { e.preventDefault(); props.onSelect(key); setContext({ ...point(e), edge: key }); }} />
                <text className="dag-priority" x={(from.x + to.x) / 2} y={(from.y + to.y) / 2 - 7}>{route.priority}</text>
              </g>;
            })}
            {drag?.type === 'wire' && <path className={`dag-preview${snap ? ' snapped' : ''}`} d={drag.end === 'right' ? wirePath(drag.fixed, snap ?? drag.moving) : wirePath(snap ?? drag.moving, drag.fixed)} />}
          </svg>
          {nodes.map(node => <article key={node.id} data-node={node.id} className={`dag-node dag-${node.kind}${node.enabled ? '' : ' disabled'}`} style={{ left: node.x, top: node.y, width: node.width, height: node.height }}
            onContextMenu={e => { if (node.kind === 'alias') { e.preventDefault(); setContext({ ...point(e), node: node.id }); } }}>
            <header className="dag-node-header" onPointerDown={event => {
              if (event.button !== 0) return; capture(event); setContext(null);
              const pos = point(event); setDrag({ type: 'node', id: node.id, offset: { x: pos.x - node.x, y: pos.y - node.y }, point: { x: node.x, y: node.y } });
            }}>
              <span className="dag-kind">{t(node.kind === 'entry' ? 'dag.entry' : node.kind === 'provider' ? 'dag.provider' : 'dag.alias')}{!node.enabled && ` · ${t('entry.disabled')}`}</span>
              <strong title={node.id}>{node.label}</strong>
              {node.kind !== 'alias' && <small title={node.detail || node.id}>{node.kind === 'entry' && node.strategy ? t(`entry.${node.strategy}` as 'entry.failover') : node.detail ? `${node.detail} · ${node.id}` : node.id}</small>}
            </header>
            {node.kind === 'provider' && <div className="dag-models">{node.models.map((model, index) => <div key={model ?? '__passthrough'} title={model ?? t('entry.keepModel')} style={{ top: 90 + index * 30 }}>{model ?? <em>{t('dag.passthrough')}</em>}</div>)}</div>}
            {nodePorts(node).map(port => <button type="button" key={portKey(port)} className={`dag-port ${port.direction}${samePort(snap, port) ? ' snapped' : ''}`} style={{ left: port.x - node.x, top: port.y - node.y }}
              aria-label={`${t(port.direction === 'input' ? 'dag.input' : 'dag.output')} · ${node.id}${port.model ? ` / ${port.model}` : ''}`}
              title={`${node.id}${port.model ? ` / ${port.model}` : ''}`}
              onPointerDown={event => startWire(port, event)}
              onClick={event => {
                if (event.detail !== 0) return; // keyboard equivalent: select a port, then its opposite
                const active = dragRef.current;
                if (active?.type === 'wire' && active.fixed.id !== port.id && port.direction === (active.end === 'right' ? 'input' : 'output')) finishWire(active, port);
                else startWire(port);
              }} />)}
          </article>)}
          {selectedRoute && (() => {
            const from = outputPort(selectedRoute), to = inputPort(selectedRoute); if (!from || !to) return null;
            return <>
              <button className="dag-reconnect" type="button" style={{ left: from.x + 15, top: from.y }} aria-label={t('dag.reconnectSource')} onPointerDown={e => startWire(to, e, props.selected!, 'left')} onClick={e => { if (e.detail === 0) startWire(to, undefined, props.selected!, 'left'); }}>↗</button>
              <button className="dag-reconnect" type="button" style={{ left: to.x - 15, top: to.y }} aria-label={t('dag.reconnectTarget')} onPointerDown={e => startWire(from, e, props.selected!, 'right')} onClick={e => { if (e.detail === 0) startWire(from, undefined, props.selected!, 'right'); }}>↘</button>
            </>;
          })()}
          {context && <div className="dag-context" role="menu" style={{ left: context.x, top: context.y }}>
            <button type="button" role="menuitem" onClick={() => { if (context.edge) props.onDelete(context.edge); if (context.node) props.onDeleteNode(context.node); setContext(null); }}>{context.node ? t('dag.deleteNode') : t('common.delete')}</button>
            <button type="button" role="menuitem" onClick={() => setContext(null)}>{t('common.cancel')}</button>
          </div>}
        </div>
      </div>
    </div>
    {selectedRoute && <div className="dag-inspector"><EdgeInspector route={selectedRoute} style={{ position: 'relative', width: '100%' }} onPatch={patch => props.onPatch(props.selected!, patch)} onDelete={() => props.onDelete(props.selected!)} onClose={() => props.onSelect(null)} /></div>}
    {!nodes.length && <p className="dag-empty">{t('dag.empty')}</p>}
  </section>;
}
