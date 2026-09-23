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
  /**
   * 建边。`model` 来源 = 拖放命中的端口：
   * - 命中端口 → model=该端口模型（或 null=透传端口）→ 写入/不写 upstream_model
   * - 命中卡片空白 → model=undefined → exact 同名钉死（daemon 行为：转发 model=left）
   */
  onCreateEdge: (left: string, right: string, model?: string | null) => void;
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

/** 右列：提供商卡片（内嵌模型端口）或别名平节点 */
interface RightCard extends Box {
  ports: EndpointRow[];
}

interface EndpointRow {
  /** `${provider}|${model ?? '*'}`（'*' = 透传端口） */
  key: string;
  model: string | null;
  pinned: boolean;
  /** 端口行几何（画布坐标） */
  x: number;
  y: number;
  w: number;
  h: number;
}

const NODE_W = 196;
const NODE_H = 52;
const CARD_W = 236;
const CARD_HEAD_H = 40;
const PORT_STEP = 28;
const PORT_H = 24;
const CARD_PB = 6;
const GAP = 16;
const PAD = 24;

/**
 * 可调用端点判定（docs/11 §一，与 daemon 转发行为对齐）：
 * - `match=prefix` 且无 upstream_model → **透传端点**（转发 model=调用方入参）
 * - 其余 → **钉死端点**，模型 = `upstream_model ?? left`（exact 无改写时 daemon 转发 model=left）
 */
export function endpointOfRoute(r: Route): { provider: string; model: string | null } {
  if (r.match === 'prefix' && !r.upstream_model) {
    return { provider: r.right, model: null };
  }
  return { provider: r.right, model: r.upstream_model ?? r.left };
}

export function endpointKey(provider: string, model: string | null): string {
  return `${provider}|${model ?? '*'}`;
}

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

function shortModel(m: string): string {
  return m.length > 18 ? `${m.slice(0, 17)}…` : m;
}

/**
 * 二部图画布（design/01 §6.2 + docs/11 §四）：SVG 边层 + DOM 节点，禁重型图库。
 *
 * 端点语义（docs/11）：右列 = **提供商大卡片**（锚定 API 地址 + key 的归属主体），
 * 卡片内每个模型 = 一个**可调用端点**（再锚定模型 id → 三元组固定）；
 * 连线锚在**端口**上，表达「模型端点 → 模型端点」的映射。
 * 拖放：命中端口 → 钉死/透传按端口；命中卡片空白 → exact 同名钉死。
 * 点线选中 → EdgeInspector（upstream_model 四字段）；键盘增删改由 RouteTableForm 等价提供。
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

  /** 每个提供商的端点集合（钉死按模型名排序，透传端口殿后；从 routes 推导 = 可调用即端点） */
  const endpointsByProvider = useMemo(() => {
    const raw = new Map<string, Map<string, { model: string | null; pinned: boolean }>>();
    for (const r of routes) {
      if (!providerIds.has(r.right)) continue;
      const ep = endpointOfRoute(r);
      const k = endpointKey(ep.provider, ep.model);
      let inner = raw.get(ep.provider);
      if (!inner) {
        inner = new Map();
        raw.set(ep.provider, inner);
      }
      inner.set(k, { model: ep.model, pinned: ep.model !== null });
    }
    const sorted = new Map<string, Map<string, { model: string | null; pinned: boolean }>>();
    for (const [pid, inner] of raw) {
      const entries = [...inner.entries()].sort(([ka, a], [kb, b]) =>
        a.pinned !== b.pinned ? (a.pinned ? -1 : 1) : ka.localeCompare(kb),
      );
      sorted.set(pid, new Map(entries));
    }
    return sorted;
  }, [routes, providerIds]);

  /* 右列布局：卡片高度随端口数变化，先算高度需求再定位 */
  const rightItems = useMemo(() => {
    return rightIds.map((id) => {
      const p = providers.find((x) => x.id === id);
      if (!p) {
        return { id, isProvider: false as const, enabled: false, h: NODE_H, ports: [] as EndpointRow[] };
      }
      const inner = endpointsByProvider.get(id);
      const eps: { key: string; model: string | null; pinned: boolean }[] = inner
        ? [...inner.entries()].map(([k, v]) => ({ key: k, ...v }))
        : [];
      const n = Math.max(1, eps.length);
      const h = CARD_HEAD_H + n * PORT_STEP + CARD_PB;
      return { id, isProvider: true as const, enabled: p.enabled, h, eps };
    });
  }, [rightIds, providers, endpointsByProvider]);

  const leftColH = leftIds.length * (NODE_H + GAP) - GAP;
  const rightStackH = rightItems.reduce((s, it) => s + it.h, 0) + Math.max(0, rightItems.length - 1) * GAP;
  const height = Math.max(380, leftColH, rightStackH) + PAD * 2;
  const width = size.w;

  const leftBoxes: Box[] = useMemo(() => {
    const y0 = Math.max(PAD, (height - leftColH) / 2);
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
  }, [leftIds, height, leftColH, aliasTargets]);

  const rightCards: RightCard[] = useMemo(() => {
    const y0 = Math.max(PAD, (height - rightStackH) / 2);
    let y = y0;
    const cardX = width - PAD - CARD_W;
    const out: RightCard[] = [];
    for (const it of rightItems) {
      const base: RightCard = {
        id: it.id,
        x: cardX,
        y,
        w: it.isProvider ? CARD_W : NODE_W,
        h: it.h,
        badge: it.isProvider ? 'PROVIDER' : 'ALIAS',
        isProvider: it.isProvider,
        enabled: it.enabled,
        ports: [],
      };
      if (it.isProvider) {
        const eps = (it as { eps: { key: string; model: string | null; pinned: boolean }[] }).eps;
        if (eps.length === 0) {
          // 无端口：占位行（卡片仍是拖放目标 → 落卡片=同名钉死建边）
          base.ports = [];
        } else {
          base.ports = eps.map((e, i) => ({
            key: e.key,
            model: e.model,
            pinned: e.pinned,
            x: cardX + 6,
            y: y + CARD_HEAD_H + 2 + i * PORT_STEP,
            w: CARD_W - 12,
            h: PORT_H,
          }));
        }
      }
      out.push(base);
      y += it.h + GAP;
    }
    return out;
  }, [rightItems, height, rightStackH, width]);

  /** 端口锚点索引：endpointKey → {x: 端口左边, y: 端口中心} */
  const portAnchors = useMemo(() => {
    const m = new Map<string, { x: number; y: number }>();
    for (const c of rightCards) {
      for (const p of c.ports) {
        m.set(p.key, { x: p.x, y: p.y + p.h / 2 });
      }
    }
    return m;
  }, [rightCards]);

  /* 边几何：出边按 priority 错开；入边锚到**端口**（同一端口的多边错开） */
  const edges = useMemo(() => {
    const outIndex = new Map<string, Route[]>();
    for (const r of routes) {
      const arr = outIndex.get(r.left) ?? [];
      arr.push(r);
      outIndex.set(r.left, arr);
    }
    const inIndex = new Map<string, Route[]>();
    for (const r of routes) {
      const isCard = providerIds.has(r.right);
      const gk = isCard
        ? endpointKey(endpointOfRoute(r).provider, endpointOfRoute(r).model)
        : `alias:${r.right}`;
      const arr = inIndex.get(gk) ?? [];
      arr.push(r);
      inIndex.set(gk, arr);
    }

    return routes.map((r) => {
      const fromBox = leftBoxes.find((b) => b.id === r.left);
      const toCard = rightCards.find((c) => c.id === r.right);
      const key = edgeKey(r.left, r.right);

      const outs = (outIndex.get(r.left) ?? [])
        .slice()
        .sort((a, b) => a.priority - b.priority);
      const ep = endpointOfRoute(r);
      const gk = providerIds.has(r.right) ? endpointKey(ep.provider, ep.model) : `alias:${r.right}`;
      const ins = (inIndex.get(gk) ?? [])
        .slice()
        .sort((a, b) => (a.left < b.left ? -1 : a.left > b.left ? 1 : a.priority - b.priority));

      const outOff = offsetsFor(outs.length)[outs.findIndex((x) => edgeKey(x.left, x.right) === key)] ?? 0;
      const inOff = offsetsFor(ins.length)[ins.findIndex((x) => edgeKey(x.left, x.right) === key)] ?? 0;

      // 终点：卡片 → 端口锚点；别名 → 节点中心
      let tx = 0;
      let ty = 0;
      let targetOk = false;
      if (toCard?.isProvider) {
        const anchor = portAnchors.get(endpointKey(ep.provider, ep.model));
        if (anchor) {
          tx = anchor.x;
          ty = anchor.y + inOff;
          targetOk = true;
        }
      } else if (toCard) {
        tx = toCard.x;
        ty = toCard.y + toCard.h / 2 + inOff;
        targetOk = true;
      }

      if (!fromBox || !targetOk) {
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
  }, [routes, leftBoxes, rightCards, portAnchors, providerIds]);

  /* 拖线：pointermove 跟随；pointerup 优先命中端口，其次卡片 */
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
      const epEl = target?.closest?.('[data-endpoint]') as HTMLElement | null;
      const rightEl = target?.closest?.('[data-right-node]') as HTMLElement | null;
      if (epEl) {
        const ep = epEl.getAttribute('data-endpoint') ?? '';
        const sep = ep.indexOf('|');
        const provider = ep.slice(0, sep);
        const modelRaw = ep.slice(sep + 1);
        onCreateEdge(drag.leftId, provider, modelRaw === '*' ? null : modelRaw);
      } else if (rightEl) {
        // 卡片空白：exact 同名钉死（model=undefined → 不写 upstream_model）
        onCreateEdge(drag.leftId, rightEl.getAttribute('data-right-node') ?? '');
      }
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
    const edge = edges.find((x) => x.key === selectedEdge);
    return edge && edge.visible ? { mx: edge.mx, my: edge.my } : null;
  }, [selectedRoute, selectedEdge, edges]);

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
      {/* 左列：网关侧模型端点（公共 id；Tab 序 = 视觉序：左列 → 边 → 右列） */}
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
            <span className="min-w-0 truncate font-mono text-xs font-medium text-ink tabular" title={b.id}>
              {b.id}
            </span>
            <span
              className={
                'shrink-0 border px-1 py-0.5 font-mono text-[10px] uppercase tracking-widest ' +
                (b.badge === 'PREFIX' || b.badge === 'ALIAS'
                  ? 'border-border text-inkMuted'
                  : 'border-ink text-ink')
              }
            >
              {b.badge}
            </span>
          </div>
          {/* 拖线锚点（aria 保持稳定——CDP/自动化以该文案定位） */}
          <button
            type="button"
            aria-label={`从 ${b.id} 拖出连线到提供商`}
            title="拖到右侧卡片内的模型端口建端点边；落卡片空白 = 同名钉死"
            onPointerDown={(e) => startDrag(b.id, e)}
            className="absolute -right-1.5 top-1/2 h-3 w-3 -translate-y-1/2 cursor-crosshair border border-ink bg-panel hover:bg-ink"
            style={{ touchAction: 'none' }}
          />
        </div>
      ))}

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
              // 端点可读性：钉死边标注目标模型，透传边标注 *input*（docs/11 §四）
              const parts = [`p=${e.route.priority}`];
              if (e.route.upstream_model) parts.push(shortModel(e.route.upstream_model));
              else if (e.route.match === 'prefix') parts.push('*input*');
              if (sticky) parts.push('sticky');
              const label = parts.join(' · ');
              const bw = label.length * 7.2 + 14;
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
                      y={4}
                      className="fill-ink"
                      style={{
                        fontSize: 12,
                        fontFamily: 'var(--font-mono)',
                        fontVariantNumeric: 'tabular-nums',
                      }}
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

      {/* 右列：提供商卡片（内嵌模型端口）/ 别名平节点 */}
      {rightCards.map((b) =>
        b.isProvider ? (
          <div
            key={`R-${b.id}`}
            data-right-node={b.id}
            tabIndex={0}
            role="group"
            aria-label={`provider ${b.id}, ${b.ports.length} endpoints`}
            className="absolute z-10 border border-border bg-panel focus-visible:outline focus-visible:outline-1"
            style={{ left: b.x, top: b.y, width: b.w, height: b.h }}
            onClick={(e) => e.stopPropagation()}
          >
            {/* 卡头 = 提供商锚定：地址 + key 归属（API token 在 provider 层，掩码展示归 Providers 页） */}
            <div className="flex h-10 items-center justify-between gap-2 border-b border-border px-2.5">
              <span className="flex min-w-0 items-center gap-2">
                <span
                  className={
                    'inline-block h-1.5 w-1.5 shrink-0 rounded-full ' +
                    (b.enabled ? 'bg-ok' : 'bg-inkSubtle')
                  }
                  aria-hidden
                />
                <span className="min-w-0 truncate font-mono text-xs font-medium text-ink tabular" title={b.id}>
                  {b.id}
                </span>
              </span>
              <span className="flex shrink-0 items-center gap-1">
                <span className="border border-border px-1 py-0.5 font-mono text-[10px] uppercase tracking-widest text-inkSubtle tabular">
                  {b.ports.length} ep
                </span>
                <span className="border border-border px-1 py-0.5 font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
                  PROVIDER
                </span>
              </span>
            </div>
            {/* 端口区 = 卡片内的模型端点（三元组再锚定模型 id） */}
            <div className="flex flex-col gap-1 px-1.5 pt-1.5">
              {b.ports.length === 0 ? (
                <div className="flex h-6 items-center border border-dashed border-border px-1.5 font-mono text-[10px] text-inkSubtle">
                  no endpoint — 拖线到此卡片建同名端点
                </div>
              ) : (
                b.ports.map((p) => (
                  <div
                    key={p.key}
                    data-endpoint={p.key}
                    data-right-node={b.id}
                    role="group"
                    aria-label={`endpoint ${b.id}/${p.model ?? 'passthrough'}`}
                    className="flex h-6 items-center gap-1.5 border border-border bg-bg px-1.5 font-mono text-[10px] text-ink"
                    title={
                      p.pinned
                        ? `钉死端点：(${b.id}) ${p.model} — 地址×模型×token 三固定`
                        : `透传端点：模型 = 调用方入参（local/* → local/qwen…）`
                    }
                  >
                    <span
                      className={
                        'inline-block h-1 w-1 shrink-0 ' + (p.pinned ? 'bg-ink' : 'bg-inkSubtle')
                      }
                      aria-hidden
                    />
                    <span className="min-w-0 truncate tabular" title={p.model ?? undefined}>
                      {p.model ?? '* input model'}
                    </span>
                    <span
                      className={
                        'ml-auto shrink-0 border px-1 uppercase tracking-widest ' +
                        (p.pinned
                          ? 'border-ink text-ink'
                          : 'border-border text-inkMuted')
                      }
                    >
                      {p.pinned ? 'pin' : 'pass'}
                    </span>
                  </div>
                ))
              )}
            </div>
          </div>
        ) : (
          <div
            key={`R-${b.id}`}
            data-right-node={b.id}
            tabIndex={0}
            role="group"
            aria-label={`alias ${b.id}`}
            className="absolute z-10 border border-border bg-panel px-2.5 py-2 focus-visible:outline focus-visible:outline-1"
            style={{ left: b.x, top: b.y, width: b.w, height: b.h }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex h-full items-center justify-between gap-2">
              <span className="min-w-0 truncate font-mono text-xs font-medium text-ink tabular" title={b.id}>
                {b.id}
              </span>
              <span className="shrink-0 border border-border px-1 py-0.5 font-mono text-[10px] uppercase tracking-widest text-inkMuted">
                ALIAS
              </span>
            </div>
          </div>
        ),
      )}

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
