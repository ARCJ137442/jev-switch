import { useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react';
import { edgeKey, type Route } from '../../api/admin';
import { EdgeInspector } from './EdgeInspector';

export interface CanvasProvider {
  id: string;
  enabled: boolean;
}

interface Props {
  routes: Route[];
  /** 左列对外 model id（page 推导：models fixture ∪ routes.left） */
  leftIds: string[];
  /** 右列 provider 集（启用灯）；非 provider 的 right 归入中间节点 */
  providers: CanvasProvider[];
  errorEdges: ReadonlySet<string>;
  selectedEdge: string | null;
  selectedRoute: Route | null;
  onSelectEdge: (key: string | null) => void;
  onCreateEdge: (left: string, right: string) => void;
  onPatchEdge: (key: string, patch: Partial<Route>) => void;
  onDeleteEdge: (key: string) => void;
}

interface Box {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
  badge: string;
  isProvider: boolean;
  enabled: boolean;
}

const NODE_W = 196;
const NODE_H = 52;
const GAP = 16;
const PAD = 24;

/** 同锚点多条边的纵向错开（按 priority 排序后均分） */
function offsetsFor(count: number): number[] {
  if (count <= 1) return [0];
  const usable = 22;
  const step = usable / (count - 1);
  return Array.from({ length: count }, (_, i) => -usable / 2 + i * step);
}

function cubicAt(p0: number, p1: number, p2: number, p3: number, t = 0.5): number {
  const mt = 1 - t;
  return mt * mt * mt * p0 + 3 * mt * mt * t * p1 + 3 * mt * t * t * p2 + t * t * t * p3;
}

/**
 * 二部图画布（design/01 §6.2）：SVG 边层 + DOM 节点，禁重型图库。
 * 拖线建边 → EdgeInspector 浮层；点线选中（hover 2px）；边 badge p=…。
 * Tab 可聚焦节点/边，Enter 选边；键盘增删改由 RouteTableForm 等价提供。
 */
export function BipartiteCanvas({
  routes,
  leftIds,
  providers,
  errorEdges,
  selectedEdge,
  selectedRoute,
  onSelectEdge,
  onCreateEdge,
  onPatchEdge,
  onDeleteEdge,
}: Props) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 860, h: 400 });
  const [hoverEdge, setHoverEdge] = useState<string | null>(null);
  const [drag, setDrag] = useState<{ leftId: string; x: number; y: number; ax: number; ay: number } | null>(null);

  /* 画布尺寸跟随容器 */
  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const apply = () => {
      const w = el.clientWidth;
      setSize((prev) => ({ ...prev, w: Math.max(560, w) }));
    };
    apply();
    const ro = new ResizeObserver(apply);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  /* 节点集合 */
  const providerIds = useMemo(() => new Set(providers.map((p) => p.id)), [providers]);
  const rightIds = useMemo(() => {
    const out: string[] = [];
    const seen = new Set<string>();
    for (const p of providers) {
      if (!seen.has(p.id)) {
        seen.add(p.id);
        out.push(p.id);
      }
    }
    for (const r of routes) {
      if (!providerIds.has(r.right) && !seen.has(r.right)) {
        seen.add(r.right);
        out.push(r.right);
      }
    }
    return out;
  }, [routes, providers, providerIds]);

  const aliasTargets = useMemo(() => new Set(routes.map((r) => r.right)), [routes]);

  const height = Math.max(380, Math.max(leftIds.length, rightIds.length) * (NODE_H + GAP) - GAP + PAD * 2);
  const width = size.w;

  const leftBoxes: Box[] = useMemo(() => {
    const colH = leftIds.length * (NODE_H + GAP) - GAP;
    const y0 = Math.max(PAD, (height - colH) / 2);
    return leftIds.map((id, i) => {
      const isAlias = aliasTargets.has(id);
      const isPrefix = id.includes('*');
      return {
        id,
        x: PAD,
        y: y0 + i * (NODE_H + GAP),
        w: NODE_W,
        h: NODE_H,
        badge: isAlias ? 'ALIAS' : isPrefix ? 'PREFIX' : 'MODEL',
        isProvider: false,
        enabled: false,
      };
    });
  }, [leftIds, height, aliasTargets]);

  const rightBoxes: Box[] = useMemo(() => {
    const colH = rightIds.length * (NODE_H + GAP) - GAP;
    const y0 = Math.max(PAD, (height - colH) / 2);
    return rightIds.map((id, i) => {
      const p = providers.find((x) => x.id === id);
      return {
        id,
        x: width - PAD - NODE_W,
        y: y0 + i * (NODE_H + GAP),
        w: NODE_W,
        h: NODE_H,
        badge: p ? 'PROVIDER' : 'ALIAS',
        isProvider: p !== undefined,
        enabled: p?.enabled ?? false,
      };
    });
  }, [rightIds, height, width, providers]);

  /* 边几何：出边按 priority 错开，入边按 left 序错开 */
  const edges = useMemo(() => {
    const outIndex = new Map<string, Route[]>();
    for (const r of routes) {
      const arr = outIndex.get(r.left) ?? [];
      arr.push(r);
      outIndex.set(r.left, arr);
    }
    const inIndex = new Map<string, Route[]>();
    for (const r of routes) {
      const arr = inIndex.get(r.right) ?? [];
      arr.push(r);
      inIndex.set(r.right, arr);
    }

    return routes.map((r) => {
      const fromBox = leftBoxes.find((b) => b.id === r.left);
      const toBox = rightBoxes.find((b) => b.id === r.right);
      const key = edgeKey(r.left, r.right);

      const outs = (outIndex.get(r.left) ?? [])
        .slice()
        .sort((a, b) => a.priority - b.priority);
      const ins = (inIndex.get(r.right) ?? [])
        .slice()
        .sort((a, b) => (a.left < b.left ? -1 : a.left > b.left ? 1 : a.priority - b.priority));

      const outOff = offsetsFor(outs.length)[outs.findIndex((x) => edgeKey(x.left, x.right) === key)] ?? 0;
      const inOff = offsetsFor(ins.length)[ins.findIndex((x) => edgeKey(x.left, x.right) === key)] ?? 0;

      if (!fromBox || !toBox) {
        return {
          key,
          route: r,
          fx: 0,
          fy: 0,
          tx: 0,
          ty: 0,
          c1x: 0,
          c2x: 0,
          mx: 0,
          my: 0,
          d: '',
          visible: false,
        };
      }
      const fx = fromBox.x + fromBox.w;
      const fy = fromBox.y + fromBox.h / 2 + outOff;
      const tx = toBox.x;
      const ty = toBox.y + toBox.h / 2 + inOff;
      const c1x = fx + Math.max(48, (tx - fx) / 2);
      const c2x = tx - Math.max(48, (tx - fx) / 2);
      const mx = cubicAt(fx, c1x, c2x, tx, 0.5);
      const my = cubicAt(fy, fy, ty, ty, 0.5);
      return {
        key,
        route: r,
        fx,
        fy,
        tx,
        ty,
        c1x,
        c2x,
        mx,
        my,
        d: `M ${fx} ${fy} C ${c1x} ${fy}, ${c2x} ${ty}, ${tx} ${ty}`,
        visible: true,
      };
    });
  }, [routes, leftBoxes, rightBoxes]);

  /* 拖线：pointermove 跟随，pointerup 命中右节点即建边 */
  useEffect(() => {
    if (!drag) return;
    const toLocal = (e: PointerEvent) => {
      const rect = wrapRef.current?.getBoundingClientRect();
      if (!rect) return null;
      return { x: e.clientX - rect.left, y: e.clientY - rect.top };
    };
    const onMove = (e: PointerEvent) => {
      const pt = toLocal(e);
      if (pt) setDrag((d) => (d ? { ...d, x: pt.x, y: pt.y } : d));
    };
    const onUp = (e: PointerEvent) => {
      const target = document.elementFromPoint(e.clientX, e.clientY);
      const rightEl = target?.closest?.('[data-right-node]') as HTMLElement | null;
      const rightId = rightEl?.getAttribute('data-right-node');
      if (rightId) onCreateEdge(drag.leftId, rightId);
      setDrag(null);
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  }, [drag, onCreateEdge]);

  const startDrag = (leftId: string, e: ReactPointerEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const rect = wrapRef.current?.getBoundingClientRect();
    const box = leftBoxes.find((b) => b.id === leftId);
    if (!rect || !box) return;
    const ax = box.x + box.w;
    const ay = box.y + box.h / 2;
    setDrag({ leftId, x: e.clientX - rect.left, y: e.clientY - rect.top, ax, ay });
  };

  const selectedBox = useMemo(() => {
    if (!selectedRoute) return null;
    const fromBox = leftBoxes.find((b) => b.id === selectedRoute.left);
    const toBox = rightBoxes.find((b) => b.id === selectedRoute.right);
    if (!fromBox || !toBox) return null;
    const edge = edges.find((x) => x.key === selectedEdge);
    return edge && edge.visible ? { mx: edge.mx, my: edge.my } : null;
  }, [selectedRoute, selectedEdge, leftBoxes, rightBoxes, edges]);

  const inspectorPos = selectedBox
    ? {
        left: Math.min(Math.max(8, selectedBox.mx + 14), width - 280),
        top: Math.min(Math.max(8, selectedBox.my + 14), height - 268),
      }
    : null;

  return (
    <div
      ref={wrapRef}
      className="relative w-full overflow-hidden bg-panel select-none"
      style={{ height }}
      onClick={() => onSelectEdge(null)}
    >
      {/* SVG 边层 */}
      <svg width={width} height={height} className="absolute inset-0" aria-hidden={false}>
        <defs>
          <marker id="arrow-idle" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto">
            <path d="M0,0 L8,4 L0,8 Z" className="fill-edgeIdle" />
          </marker>
          <marker id="arrow-active" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto">
            <path d="M0,0 L8,4 L0,8 Z" className="fill-edge" />
          </marker>
          <marker id="arrow-danger" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto">
            <path d="M0,0 L8,4 L0,8 Z" className="fill-danger" />
          </marker>
        </defs>

        <g role="list" aria-label="routes">
          {edges
            .filter((e) => e.visible)
            .map((e) => {
              const active = selectedEdge === e.key || hoverEdge === e.key;
              const isErr = errorEdges.has(e.key);
              const sticky = e.route.sticky === 'session';
              const label = sticky ? `p=${e.route.priority} · sticky` : `p=${e.route.priority}`;
              const bw = label.length * 5.8 + 12;
              return (
                <g key={e.key} role="listitem">
                  {/* 可命中的宽 hit 区 */}
                  <path
                    d={e.d}
                    fill="none"
                    stroke="transparent"
                    strokeWidth={16}
                    style={{ pointerEvents: 'stroke', cursor: 'pointer' }}
                    tabIndex={0}
                    role="button"
                    aria-label={`route ${e.route.left} to ${e.route.right}, priority ${e.route.priority}`}
                    onClick={(ev) => {
                      ev.stopPropagation();
                      onSelectEdge(e.key);
                    }}
                    onKeyDown={(ev) => {
                      if (ev.key === 'Enter') {
                        ev.preventDefault();
                        onSelectEdge(e.key);
                      }
                    }}
                    onFocus={() => setHoverEdge(e.key)}
                    onBlur={() => setHoverEdge(null)}
                    onMouseEnter={() => setHoverEdge(e.key)}
                    onMouseLeave={() => setHoverEdge((k) => (k === e.key ? null : k))}
                  />
                  {/* 可见边 */}
                  <path
                    d={e.d}
                    fill="none"
                    className={
                      isErr
                        ? 'stroke-danger'
                        : active
                          ? 'stroke-edge'
                          : 'stroke-edgeIdle'
                    }
                    strokeWidth={isErr || active ? 2 : 1.5}
                    markerEnd={
                      isErr ? 'url(#arrow-danger)' : active ? 'url(#arrow-active)' : 'url(#arrow-idle)'
                    }
                    style={{ pointerEvents: 'none' }}
                  />
                  {/* 边 badge */}
                  <g transform={`translate(${e.mx},${e.my})`} style={{ pointerEvents: 'none' }}>
                    <rect
                      x={-bw / 2}
                      y={-8}
                      width={bw}
                      height={16}
                      className="fill-panel"
                      stroke={isErr ? '#dc2626' : active ? '#0a0a0a' : '#e5e5e5'}
                      strokeWidth={1}
                    />
                    <text
                      textAnchor="middle"
                      y={3.5}
                      className="fill-ink"
                      style={{ fontSize: 9, fontFamily: 'var(--font-mono)' }}
                    >
                      {label}
                    </text>
                  </g>
                </g>
              );
            })}
        </g>

        {/* 拖线临时边 */}
        {drag && (
          <path
            d={`M ${drag.ax} ${drag.ay} C ${drag.ax + 60} ${drag.ay}, ${drag.x - 60} ${drag.y}, ${drag.x} ${drag.y}`}
            fill="none"
            className="stroke-edge"
            strokeWidth={1.5}
            strokeDasharray="4 3"
            style={{ pointerEvents: 'none' }}
          />
        )}
      </svg>

      {/* 左列节点 */}
      {leftBoxes.map((b) => (
        <div
          key={`L-${b.id}`}
          tabIndex={0}
          role="group"
          aria-label={`model ${b.id}, ${b.badge}`}
          className="absolute z-10 border border-border bg-panel px-2.5 py-2 focus-visible:outline focus-visible:outline-1"
          style={{ left: b.x, top: b.y, width: b.w, height: b.h }}
          onClick={(e) => e.stopPropagation()}
        >
          <div className="flex h-full items-center justify-between gap-2">
            <span className="min-w-0 truncate font-mono text-xs font-medium text-ink" title={b.id}>
              {b.id}
            </span>
            <span
              className={
                'shrink-0 border px-1 py-0.5 font-mono text-[9px] uppercase tracking-widest ' +
                (b.badge === 'PREFIX' || b.badge === 'ALIAS'
                  ? 'border-border text-inkMuted'
                  : 'border-ink text-ink')
              }
            >
              {b.badge}
            </span>
          </div>
          {/* 拖线锚点 */}
          <button
            type="button"
            aria-label={`从 ${b.id} 拖出连线到提供商`}
            title="拖到右侧提供商建边"
            onPointerDown={(e) => startDrag(b.id, e)}
            className="absolute -right-1.5 top-1/2 h-3 w-3 -translate-y-1/2 cursor-crosshair border border-ink bg-panel hover:bg-ink"
            style={{ touchAction: 'none' }}
          />
        </div>
      ))}

      {/* 右列节点 */}
      {rightBoxes.map((b) => (
        <div
          key={`R-${b.id}`}
          data-right-node={b.id}
          tabIndex={0}
          role="group"
          aria-label={b.isProvider ? `provider ${b.id}` : `alias ${b.id}`}
          className="absolute z-10 border border-border bg-panel px-2.5 py-2 focus-visible:outline focus-visible:outline-1"
          style={{ left: b.x, top: b.y, width: b.w, height: b.h }}
          onClick={(e) => e.stopPropagation()}
        >
          <div className="flex h-full items-center justify-between gap-2">
            <span className="flex min-w-0 items-center gap-2">
              {b.isProvider && (
                <span
                  className={
                    'inline-block h-1.5 w-1.5 shrink-0 rounded-full ' +
                    (b.enabled ? 'bg-ok' : 'bg-inkSubtle')
                  }
                  aria-hidden
                />
              )}
              <span className="min-w-0 truncate font-mono text-xs font-medium text-ink" title={b.id}>
                {b.id}
              </span>
            </span>
            <span
              className={
                'shrink-0 border px-1 py-0.5 font-mono text-[9px] uppercase tracking-widest ' +
                (b.isProvider ? 'border-border text-inkSubtle' : 'border-border text-inkMuted')
              }
            >
              {b.badge}
            </span>
          </div>
        </div>
      ))}

      {/* 边检视器浮层 */}
      {selectedRoute && selectedEdge && inspectorPos && (
        <EdgeInspector
          route={selectedRoute}
          style={{ left: inspectorPos.left, top: inspectorPos.top }}
          onPatch={(patch) => onPatchEdge(selectedEdge, patch)}
          onDelete={() => onDeleteEdge(selectedEdge)}
          onClose={() => onSelectEdge(null)}
        />
      )}
    </div>
  );
}
