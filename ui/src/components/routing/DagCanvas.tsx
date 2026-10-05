import { useEffect, useId, useMemo, useRef, useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from 'react';
import { ChevronDown, ChevronUp, CornerUpLeft, CornerUpRight, Minus, Plus, RotateCcw } from 'lucide-react';
import { edgeKey, type Route } from '../../api/admin';
import { useI18n } from '../../i18n';
import { EdgeInspector } from './EdgeInspector';
import { buildDagLayout, corridorKey, nearestPort, nodePorts, routeIdentity, wheelPanDelta, wirePath, type DagEntry, type DagPort, type DagProvider, type Point } from './dag';
import './dag.css';
import { readRoutingHudAutoHide, SETTINGS_CHANGE_EVENT } from '../../settings/preferences';
import { edgeActivityVisual, type RouteActivityState } from '../../pages/routingActivity';

interface Props {
  routes: Route[]; entries: DagEntry[]; providers: DagProvider[];
  positions: Record<string, Point>; selected: string | null; selectedNode?: string | null; errors: ReadonlySet<string>;
  routeActivities?: RouteActivityState;
  activityNow: number;
  onSelect: (id: string | null) => void;
  onSelectNode?: (id: string | null) => void;
  onNodeDoubleClick?: (node: { id: string; kind: 'entry' | 'provider' | 'alias' }) => void;
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
type PanDrag = { type: 'pan'; start: Point; offset: Point; button: number };
type Drag = WireDrag | NodeDrag | PanDrag;
const samePort = (a: DagPort | null, b: DagPort) => a?.id === b.id && a.direction === b.direction && a.model === b.model;

function CanvasHud({ children, autoHide, label, placement }: { children: React.ReactNode; autoHide: boolean; label: string; placement: 'primary' | 'secondary' }) {
  const { t } = useI18n();
  const [expanded, setExpanded] = useState(() => !autoHide);
  useEffect(() => setExpanded(!autoHide), [autoHide]);
  return <div className={`dag-hud dag-hud--${placement}${expanded ? ' dag-hud--expanded' : ''}`}>
    <button className="dag-icon-button dag-hud__toggle" type="button" aria-expanded={expanded} aria-label={t(expanded ? 'dag.collapseToolbar' : 'dag.expandToolbar')} title={`${label} · ${t(expanded ? 'dag.collapseToolbar' : 'dag.expandToolbar')}`} onClick={() => setExpanded(value => !value)}>
      {expanded ? <ChevronUp size={15} aria-hidden="true" /> : <ChevronDown size={15} aria-hidden="true" />}
    </button>
    <div className="dag-hud__content">{children}</div>
  </div>;
}

/** Shared graph coordinates make snap/reconnect work even when released between DOM elements. */
export function DagCanvas(props: Props) {
  const { t } = useI18n();
  const viewport = useRef<HTMLDivElement>(null);
  const world = useRef<HTMLDivElement>(null);
  const dragRef = useRef<Drag | null>(null);
  const touchPoints = useRef(new Map<number, Point>());
  const pinchDistance = useRef<number | null>(null);
  const [drag, setDragState] = useState<Drag | null>(null);
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState<Point>({ x: 24, y: 24 });
  const [nodeId, setNodeId] = useState('');
  const [context, setContext] = useState<{ x: number; y: number; edge?: string; node?: string; background?: boolean } | null>(null);
  const [autoHideHud, setAutoHideHud] = useState(readRoutingHudAutoHide);
  const marker = useId().replace(/:/g, '');
  useEffect(() => {
    const syncPreference = () => setAutoHideHud(readRoutingHudAutoHide());
    window.addEventListener(SETTINGS_CHANGE_EVENT, syncPreference);
    return () => window.removeEventListener(SETTINGS_CHANGE_EVENT, syncPreference);
  }, []);
  const setDrag = (next: Drag | null) => { dragRef.current = next; setDragState(next); };
  const positions = drag?.type === 'node' ? { ...props.positions, [drag.id]: drag.point } : props.positions;
  const layout = useMemo(() => buildDagLayout(props.routes, props.entries, props.providers, positions), [props.routes, props.entries, props.providers, positions]);
  const nodes = layout.nodes;
  const ports = nodes.flatMap(nodePorts);
  const width = Math.max(1000, ...nodes.map(n => n.x + n.width + 80));
  const height = Math.max(460, ...nodes.map(n => n.y + n.height + 80));
  const selectedRoute = props.routes.find(r => routeIdentity(r) === props.selected);
  const snap = drag?.type === 'wire' ? nearestPort(ports, drag.moving, drag.end === 'right' ? 'input' : 'output', drag.fixed.id, 40 / zoom) : null;
  const zoomAt = (nextZoom: number, anchor: Point) => {
    const worldPoint = { x: (anchor.x - pan.x) / zoom, y: (anchor.y - pan.y) / zoom };
    setPan({ x: anchor.x - worldPoint.x * nextZoom, y: anchor.y - worldPoint.y * nextZoom });
    setZoom(nextZoom);
  };
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
    <CanvasHud autoHide={autoHideHud} label={t('dag.hint')} placement="primary">
      <div className="dag-tools">
        <span className="dag-help">{t('dag.hint')}</span>
        <div className="dag-tool-group">
          <button className="dag-icon-button" type="button" onClick={() => zoomAt(Math.max(.4, Math.round((zoom - .1) * 10) / 10), { x: (viewport.current?.clientWidth ?? 0) / 2, y: (viewport.current?.clientHeight ?? 0) / 2 })} aria-label={t('dag.zoomOut')} title={t('dag.zoomOut')}><Minus size={15}/></button>
          <output>{Math.round(zoom * 100)}%</output>
          <button className="dag-icon-button" type="button" onClick={() => zoomAt(Math.min(2.4, Math.round((zoom + .1) * 10) / 10), { x: (viewport.current?.clientWidth ?? 0) / 2, y: (viewport.current?.clientHeight ?? 0) / 2 })} aria-label={t('dag.zoomIn')} title={t('dag.zoomIn')}><Plus size={15}/></button>
          <button className="dag-icon-button" type="button" onClick={() => { props.onResetLayout(); setZoom(1); setPan({ x: 24, y: 24 }); }} aria-label={t('dag.layout')} title={t('dag.layout')}><RotateCcw size={15}/></button>
        </div>
      </div>
    </CanvasHud>
    {selectedRoute && <CanvasHud autoHide={autoHideHud} label={t('dag.insert')} placement="secondary"><div className="dag-tools">
      <label className="dag-insert"><span>{t('dag.nodeId')}</span><input value={nodeId} onChange={e => setNodeId(e.target.value)} spellCheck={false} placeholder="fallback-group" /></label>
      <button type="button" disabled={!nodeId.trim()} onClick={() => { props.onInsertNode(props.selected!, nodeId.trim()); setNodeId(''); }}>{t('dag.insert')}</button>
    </div></CanvasHud>}
    <div className="dag-viewport" ref={viewport} tabIndex={0}
      onKeyDown={event => { if (event.key === 'Escape') { setDrag(null); setContext(null); props.onSelect(null); } }}
      onPointerDown={event => {
        if (event.pointerType === 'touch') {
          touchPoints.current.set(event.pointerId, { x: event.clientX, y: event.clientY });
          if (touchPoints.current.size === 2) {
            setDrag(null);
            const points = [...touchPoints.current.values()];
            pinchDistance.current = Math.hypot(points[1].x - points[0].x, points[1].y - points[0].y);
            capture(event);
            return;
          }
        }
        if ((event.target as HTMLElement).closest('button,input,select,[data-node],.dag-edge')) return;
        setContext(null); props.onSelect(null);
        if (event.button === 1 || event.button === 2 || event.button === 0) { capture(event); setDrag({ type: 'pan', start: { x: event.clientX, y: event.clientY }, offset: pan, button: event.button }); }
      }}
      onContextMenu={event => {
        if ((event.target as HTMLElement).closest('[data-node],.dag-edge,button,input,select')) return;
        event.preventDefault();
        setContext({ ...point(event), background: true });
      }}
      onWheel={event => {
        const view = viewport.current;
        if (!view) return;
        // Let the insert field and HUD controls keep their normal wheel behavior.
        if ((event.target as HTMLElement).closest('input,select,textarea,button')) return;
        if (!event.ctrlKey && !event.metaKey) {
          event.preventDefault();
          const delta = wheelPanDelta(event.deltaX, event.deltaY, event.shiftKey);
          setPan(current => ({ x: current.x + delta.x, y: current.y + delta.y }));
          return;
        }
        event.preventDefault();
        const oldZoom = zoom;
        const nextZoom = Math.min(2.4, Math.max(.4, Math.round((oldZoom * (event.deltaY < 0 ? 1.1 : .9)) * 100) / 100));
        if (nextZoom === oldZoom) return;
        const bounds = view.getBoundingClientRect();
        zoomAt(nextZoom, { x: event.clientX - bounds.left, y: event.clientY - bounds.top });
      }}
      onPointerMove={event => {
        if (event.pointerType === 'touch' && touchPoints.current.has(event.pointerId)) {
          touchPoints.current.set(event.pointerId, { x: event.clientX, y: event.clientY });
          if (touchPoints.current.size === 2 && pinchDistance.current) {
            const points = [...touchPoints.current.values()];
            const distance = Math.hypot(points[1].x - points[0].x, points[1].y - points[0].y);
            const factor = distance / pinchDistance.current;
            if (Math.abs(factor - 1) > 0.02) {
              const nextZoom = Math.min(2.4, Math.max(.4, Math.round(zoom * factor * 100) / 100));
              if (nextZoom !== zoom) setZoom(nextZoom);
              pinchDistance.current = distance;
            }
            return;
          }
        }
        const active = dragRef.current;
        if (!active) return;
        if (active.type === 'pan') { setPan({ x: active.offset.x + event.clientX - active.start.x, y: active.offset.y + event.clientY - active.start.y }); return; }
        const pos = point(event);
        if (active.type === 'wire') setDrag({ ...active, moving: pos });
        else setDrag({ ...active, point: { x: Math.max(14, pos.x - active.offset.x), y: Math.max(34, pos.y - active.offset.y) } });
      }}
      onPointerUp={event => {
        if (event.pointerType === 'touch') {
          touchPoints.current.delete(event.pointerId);
          if (touchPoints.current.size < 2) pinchDistance.current = null;
        }
        const active = dragRef.current;
        if (!active) return;
        if (active.type === 'wire') {
          const target = nearestPort(ports, point(event), active.end === 'right' ? 'input' : 'output', active.fixed.id, 40 / zoom);
          if (target) finishWire(active, target);
        } else if (active.type === 'node') props.onMove(active.id, active.point);
        setDrag(null);
        if (viewport.current?.hasPointerCapture(event.pointerId)) viewport.current.releasePointerCapture(event.pointerId);
      }}
      onPointerCancel={event => { touchPoints.current.delete(event.pointerId); pinchDistance.current = null; setDrag(null); }}>
      <div className="dag-canvas-layer">
        <div ref={world} className="dag-world" style={{ width, height, transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})` }}>
          <svg className="dag-wires" width={width} height={height} aria-label={t('dag.connections')}>
            <defs><marker id={marker} viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="context-stroke" /></marker></defs>
            {props.routes.map((route, index) => {
              const from = outputPort(route); const to = inputPort(route); if (!from || !to) return null;
               const key = routeIdentity(route); const selected = key === props.selected;
               const activity = edgeActivityVisual(props.routeActivities?.[key], props.activityNow);
               const invalid = props.errors.has(edgeKey(route.left, route.right));
               const corridor = positions[route.left] || positions[route.right] ? [] : layout.corridors.get(corridorKey(route.left, route.right)) ?? [];
               const path = wirePath(from, to, corridor);
               const middle = corridor[Math.floor(corridor.length / 2)] ?? { x: (from.x + to.x) / 2, y: (from.y + to.y) / 2 };
               const activityStyle: CSSProperties | undefined = activity.lineLevel > 0 || activity.outcomeCount > 0 ? {
                 '--dag-edge-color': `hsl(${activity.hue} ${activity.saturation}% 72%)`,
                 '--dag-edge-opacity': String(Math.max(0.42, activity.lineLevel)),
                 '--dag-glow-alpha': String(Math.min(0.86, activity.glowLevel * 0.86)),
                 '--dag-glow-radius': `${(activity.glowLevel * 16).toFixed(1)}px`,
               } as CSSProperties : undefined;
               const activitySummary = activity.activeCount > 0 || activity.outcomeCount > 0
                 ? t('dag.activitySummary', { active: activity.activeCount, count: activity.outcomeCount, success: activity.successRate === null ? '—' : Math.round(activity.successRate * 100) })
                 : null;
               return <g key={`${key}:${index}`} className={`dag-edge${selected ? ' selected' : ''}${invalid ? ' invalid' : ''}`}
                style={activityStyle}
                role="button" tabIndex={0} aria-pressed={selected} aria-label={`${route.left} → ${route.right}${route.upstream_model ? ` / ${route.upstream_model}` : ''}${activitySummary ? ` · ${activitySummary}` : ''}`}
                onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.stopPropagation(); props.onSelect(key); } }}>
                <path className="dag-glow" d={path} />
                <path className="dag-wire" d={path} markerEnd={`url(#${marker})`} />
                <path className="dag-hit" d={path} onClick={e => { e.stopPropagation(); props.onSelect(key); }} onContextMenu={e => { e.preventDefault(); props.onSelect(key); setContext({ ...point(e), edge: key }); }} />
                <text className="dag-priority" x={middle.x} y={middle.y - 7}>{route.priority}</text>
              </g>;
            })}
            {drag?.type === 'wire' && <path className={`dag-preview${snap ? ' snapped' : ''}`} d={drag.end === 'right' ? wirePath(drag.fixed, snap ?? drag.moving) : wirePath(snap ?? drag.moving, drag.fixed)} />}
          </svg>
          {nodes.map(node => <article key={node.id} data-node={node.id} className={`dag-node dag-${node.kind}${node.enabled ? '' : ' disabled'}${props.selectedNode === node.id ? ' selected' : ''}`} style={{ left: node.x, top: node.y, width: node.width, height: node.height }}
            onClick={event => { if (!(event.target as HTMLElement).closest('button')) props.onSelectNode?.(node.id); }}
            onDoubleClick={() => props.onNodeDoubleClick?.(node)}
            onContextMenu={e => { if (node.kind === 'alias') { e.preventDefault(); setContext({ ...point(e), node: node.id }); } }}>
            <header className="dag-node-header" onPointerDown={event => {
              if (event.button !== 0) return; capture(event); setContext(null);
              const pos = point(event); setDrag({ type: 'node', id: node.id, offset: { x: pos.x - node.x, y: pos.y - node.y }, point: { x: node.x, y: node.y } });
            }}>
              <span className="dag-kind">{t(node.kind === 'entry' ? 'dag.entry' : node.kind === 'provider' ? 'dag.provider' : 'dag.alias')}{!node.enabled && ` · ${t('entry.disabled')}`}</span>
              <strong title={node.id}>{node.label}</strong>
              {node.kind !== 'alias' && <small title={node.detail || node.id}>{node.kind === 'entry' && node.strategy ? t(`entry.${node.strategy}` as 'entry.failover') : node.detail ? `${node.detail} · ${node.id}` : node.id}</small>}
              <small className="dag-node-degree">{node.inCount} in · {node.outCount} out</small>
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
              <button className="dag-reconnect" type="button" style={{ left: from.x + 15, top: from.y }} aria-label={t('dag.reconnectSource')} title={t('dag.reconnectSource')} onPointerDown={e => startWire(to, e, props.selected!, 'left')} onClick={e => { if (e.detail === 0) startWire(to, undefined, props.selected!, 'left'); }}><CornerUpLeft size={14}/></button>
              <button className="dag-reconnect" type="button" style={{ left: to.x - 15, top: to.y }} aria-label={t('dag.reconnectTarget')} title={t('dag.reconnectTarget')} onPointerDown={e => startWire(from, e, props.selected!, 'right')} onClick={e => { if (e.detail === 0) startWire(from, undefined, props.selected!, 'right'); }}><CornerUpRight size={14}/></button>
            </>;
          })()}
          {context && <div className="dag-context" role="menu" style={{ left: context.x, top: context.y }}>
            {context.background ? <button type="button" role="menuitem" onClick={() => { props.onResetLayout(); setZoom(1); setPan({ x: 24, y: 24 }); setContext(null); }}>{t('dag.layout')}</button> : <button type="button" role="menuitem" onClick={() => { if (context.edge) props.onDelete(context.edge); if (context.node) props.onDeleteNode(context.node); setContext(null); }}>{context.node ? t('dag.deleteNode') : t('common.delete')}</button>}
            <button type="button" role="menuitem" onClick={() => setContext(null)}>{t('common.cancel')}</button>
          </div>}
        </div>
      </div>
    </div>
    {selectedRoute && <div className="dag-inspector"><EdgeInspector route={selectedRoute} style={{ position: 'relative', width: '100%' }} onPatch={patch => props.onPatch(props.selected!, patch)} onDelete={() => props.onDelete(props.selected!)} onClose={() => props.onSelect(null)} /></div>}
    {!nodes.length && <p className="dag-empty">{t('dag.empty')}</p>}
  </section>;
}
