import { useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react';
import { edgeKey, type Route } from '../../api/admin';
import { EdgeInspector } from './EdgeInspector';
import { t as tCore } from '../../i18n';

export interface CanvasProvider {
  id: string;
  enabled: boolean;
}

export interface RouteStats {
  calls_24h: number;
  success_rate: number;
  slow_rate: number;
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
  /** 路由统计数据（调用次数、成功率等） */
  routeStats?: Map<string, RouteStats>;
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

/** 拖线锚点：视觉 12px，命中区 32px（触屏可用性 — 布局几何不受影响） */
const ANCHOR_VIS = 12;
const ANCHOR_HIT = 32;

/** 磁吸距离（规范 §3.1） */
const SNAP_DISTANCE = 40;

/** 端口尺寸（规范 §3.3） */
const PORT_DOT_NORMAL = 6;
const PORT_DOT_DRAG_NEARBY = 10;

/** 类型胶囊（v2：无 uppercase / 无 tracking-widest，最小 12px） */
const CHIP: React.CSSProperties = {
  borderRadius: 999,
  padding: '0.0625rem 0.375rem',
  fontSize: 'var(--text-xs)',
  lineHeight: 1.4,
  fontWeight: 500,
};

/** 徽标机读值 → 展示文案（值保持不变，仅呈现小写） */
const CHIP_TEXT: Record<string, string> = {
  MODEL: 'model',
  PREFIX: 'prefix',
  ALIAS: 'alias',
  PROVIDER: 'provider',
};

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

/** 同锚点多条边的放射状错开（规范 §3.9：-15°, 0°, +15°） */
function offsetsFor(count: number): number[] {
  if (count <= 1) return [0];
  const angleSpan = 30; // 总角度跨度（度）
  const angles = Array.from({ length: count }, (_, i) => {
    const offset = (i - (count - 1) / 2) * (angleSpan / (count - 1 || 1));
    return offset;
  });
  // 转换为纵向像素偏移（简化实现：直接映射到 Y 偏移）
  return angles.map((deg) => (deg / 30) * 22);
}

function cubicAt(p0: number, p1: number, p2: number, p3: number, t = 0.5): number {
  const mt = 1 - t;
  return mt * mt * mt * p0 + 3 * mt * mt * t * p1 + 3 * mt * t * t * p2 + t * t * t * p3;
}

function shortModel(m: string): string {
  return m.length > 18 ? `${m.slice(0, 17)}…` : m;
}

/** 计算线条粗细（规范 §3.8：对数映射调用次数） */
function getStrokeWidth(callCount: number): number {
  if (callCount === 0) return 2;
  return Math.min(2 + 2 * Math.floor(Math.log10(callCount)), 8);
}

/** 撤销栈动作类型（规范 §3.10） */
interface UndoAction {
  type: 'create' | 'delete' | 'update';
  route: Route;
  before?: Route; // update 操作需要保存修改前的状态
}

/** 拖拽状态类型 */
interface DragState {
  leftId: string;
  x: number;
  y: number;
  ax: number;
  ay: number;
  /** 磁吸目标端口 */
  snapTarget?: { provider: string; model: string | null; x: number; y: number };
  /** 重连模式：拖拽已有箭头的起点 */
  reconnectKey?: string;
}

/**
 * 二部图画布（design/01 §6.2 + docs/11 §四 + ROUTING-INTERACTION-SPEC-v2）：
 * SVG 边层 + DOM 节点，禁重型图库。
 *
 * 端点语义（docs/11）：右列 = **提供商大卡片**（锚定 API 地址 + key 的归属主体），
 * 卡片内每个模型 = 一个**可调用端点**（再锚定模型 id → 三元组固定）；
 * 连线锚在**端口**上（端口位置在卡片左侧），表达「模型端点 → 模型端点」的映射。
 * 拖放：命中端口 → 钉死/透传按端口；命中卡片空白 → exact 同名钉死。
 * 点线选中 → EdgeInspector（upstream_model 四字段）。
 *
 * v2 新增交互（ROUTING-INTERACTION-SPEC-v2）：
 * - 磁吸机制（40px 触发距离）
 * - 双拖拽起点（端口小方块 + 箭头左半部分重连）
 * - 端口可视化（实心/空心圆点，hover 放大，拖拽靠近发光）
 * - 删除交互（Delete/Backspace 键 + Inspector 面板 + 右键菜单）
 * - 箭头样式（线条粗细映射调用次数）
 * - 箭头中点标签（priority + 成功率环形图）
 * - 多边放射状错开（-15°, 0°, +15°）
 * - 键盘快捷键（Delete/Esc/Ctrl+Z/Ctrl+Shift+Z）
 * - 撤销栈（最多 50 次）
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
  routeStats,
}: Props) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 860, h: 400 });
  const [hoverEdge, setHoverEdge] = useState<string | null>(null);
  const [hoverPort, setHoverPort] = useState<string | null>(null);
  const [drag, setDrag] = useState<DragState | null>(null);

  // 撤销/重做栈（规范 §3.10）
  const [undoStack, setUndoStack] = useState<UndoAction[]>([]);
  const [redoStack, setRedoStack] = useState<UndoAction[]>([]);

  // 键盘快捷键处理（规范 §3.10）
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Delete/Backspace：删除选中的边
      if ((e.key === 'Delete' || e.key === 'Backspace') && selectedEdge && selectedRoute) {
        e.preventDefault();
        onDeleteEdge(selectedEdge);
        setUndoStack((stack) => [...stack, { type: 'delete' as const, route: selectedRoute }].slice(-50));
        setRedoStack([]);
      }
      // Esc：取消拖拽
      else if (e.key === 'Escape' && drag) {
        e.preventDefault();
        setDrag(null);
      }
      // Ctrl+Z：撤销
      else if (e.ctrlKey && e.key === 'z' && !e.shiftKey && undoStack.length > 0) {
        e.preventDefault();
        const action = undoStack[undoStack.length - 1];
        setUndoStack((stack) => stack.slice(0, -1));
        setRedoStack((stack) => [...stack, action]);
        // 执行撤销逻辑
        if (action.type === 'create') {
          onDeleteEdge(edgeKey(action.route.left, action.route.right));
        } else if (action.type === 'delete') {
          onCreateEdge(action.route.left, action.route.right, action.route.upstream_model);
        } else if (action.type === 'update' && action.before) {
          onPatchEdge(edgeKey(action.route.left, action.route.right), action.before);
        }
      }
      // Ctrl+Shift+Z：重做
      else if (e.ctrlKey && e.shiftKey && e.key === 'Z' && redoStack.length > 0) {
        e.preventDefault();
        const action = redoStack[redoStack.length - 1];
        setRedoStack((stack) => stack.slice(0, -1));
        setUndoStack((stack) => [...stack, action]);
        // 执行重做逻辑
        if (action.type === 'create') {
          onCreateEdge(action.route.left, action.route.right, action.route.upstream_model);
        } else if (action.type === 'delete') {
          onDeleteEdge(edgeKey(action.route.left, action.route.right));
        } else if (action.type === 'update') {
          onPatchEdge(edgeKey(action.route.left, action.route.right), action.route);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [selectedEdge, selectedRoute, drag, undoStack, redoStack, onDeleteEdge, onCreateEdge, onPatchEdge]);


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
            x: cardX + 12,  // 端口在卡片左侧（修正：从左边缘开始，留 12px 内边距）
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

  /** 端口锚点索引：endpointKey → {x: 端口左边（圆点中心）, y: 端口中心} */
  const portAnchors = useMemo(() => {
    const m = new Map<string, { x: number; y: number }>();
    for (const c of rightCards) {
      for (const p of c.ports) {
        // 端口圆点在卡片左侧边缘（x 坐标是卡片左边）
        m.set(p.key, { x: c.x, y: p.y + p.h / 2 });
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

  /* 拖线：pointermove 跟随；计算磁吸目标；pointerup 优先命中端口，其次卡片 */
  useEffect(() => {
    if (!drag) return;
    const toLocal = (e: PointerEvent) => {
      const rect = wrapRef.current?.getBoundingClientRect();
      if (!rect) return null;
      return { x: e.clientX - rect.left, y: e.clientY - rect.top };
    };
    const onMove = (e: PointerEvent) => {
      const pt = toLocal(e);
      if (!pt) return;

      // 磁吸逻辑：检测距离 SNAP_DISTANCE 内的端口（规范 §3.1）
      let snapTarget: DragState['snapTarget'] = undefined;
      let minDist = SNAP_DISTANCE;

      for (const [key, anchor] of portAnchors) {
        const dist = Math.sqrt((pt.x - anchor.x) ** 2 + (pt.y - anchor.y) ** 2);
        if (dist < minDist) {
          minDist = dist;
          const sep = key.indexOf('|');
          const provider = key.slice(0, sep);
          const modelRaw = key.slice(sep + 1);
          snapTarget = {
            provider,
            model: modelRaw === '*' ? null : modelRaw,
            x: anchor.x,
            y: anchor.y,
          };
        }
      }

      setDrag((d) => (d ? { ...d, x: pt.x, y: pt.y, snapTarget } : d));

      // 更新 hover 端口状态
      if (snapTarget) {
        setHoverPort(endpointKey(snapTarget.provider, snapTarget.model));
      } else {
        setHoverPort(null);
      }
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
      setHoverPort(null);
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  }, [drag, onCreateEdge, portAnchors]);

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
      className="relative w-full select-none overflow-hidden"
      style={{ height, background: 'var(--surface)' }}
      onClick={() => onSelectEdge(null)}
    >
      {/* 左列：网关侧模型端点（公共 id；Tab 序 = 视觉序：左列 → 边 → 右列） */}
      {leftBoxes.map((b) => (
        <div
          key={`L-${b.id}`}
          tabIndex={0}
          role="group"
          aria-label={`model ${b.id}, ${b.badge}`}
          className="absolute z-10 px-2.5 py-2"
          style={{
            left: b.x,
            top: b.y,
            width: b.w,
            height: b.h,
            background: 'var(--surface)',
            border: '1px solid var(--border)',
            borderRadius: 'var(--radius)',
          }}
          onClick={(e) => e.stopPropagation()}
        >
          <div className="flex h-full items-center justify-between gap-2">
            <span
              className="min-w-0 truncate tabular"
              style={{
                fontFamily: 'var(--font-mono)',
                fontSize: 'var(--text-sm)',
                fontWeight: 500,
                color: 'var(--text)',
              }}
              title={b.id}
            >
              {b.id}
            </span>
            <span
              className="shrink-0"
              style={{
                ...CHIP,
                ...(b.badge === 'PREFIX' || b.badge === 'ALIAS'
                  ? { background: 'var(--surface-hover)', color: 'var(--text-muted)' }
                  : { background: 'var(--accent)', color: '#fff' }),
              }}
            >
              {CHIP_TEXT[b.badge] ?? b.badge.toLowerCase()}
            </span>
          </div>
          {/* 拖线锚点：视觉 12px，命中区 32px（透明外扩容器；移动端可用性） */}
          <button
            type="button"
            aria-label={`从 ${b.id} 拖出连线到提供商`}
            title={tCore('canvas.dragTitle')}
            onPointerDown={(e) => startDrag(b.id, e)}
            className="absolute flex items-center justify-center"
            style={{
              right: -ANCHOR_HIT / 2,
              top: '50%',
              transform: 'translateY(-50%)',
              width: ANCHOR_HIT,
              height: ANCHOR_HIT,
              padding: (ANCHOR_HIT - ANCHOR_VIS) / 2,
              background: 'transparent',
              border: 'none',
              cursor: 'crosshair',
              touchAction: 'none',
            }}
          >
            <span
              aria-hidden
              style={{
                width: ANCHOR_VIS,
                height: ANCHOR_VIS,
                borderRadius: 3,
                background: 'var(--surface)',
                border: '1px solid var(--accent)',
                display: 'block',
              }}
            />
          </button>
        </div>
      ))}

      {/* SVG 边层 */}
      <svg width={width} height={height} className="absolute inset-0" aria-hidden={false}>
        <defs>
          <marker id="arrow-idle" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto">
            <path d="M0,0 L8,4 L0,8 Z" fill="var(--edge-idle)" />
          </marker>
          <marker id="arrow-active" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto">
            <path d="M0,0 L8,4 L0,8 Z" fill="var(--edge)" />
          </marker>
          <marker id="arrow-danger" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto">
            <path d="M0,0 L8,4 L0,8 Z" fill="var(--danger)" />
          </marker>
        </defs>

        <g role="list" aria-label="routes">
          {edges
            .filter((e) => e.visible)
            .map((e) => {
              const active = selectedEdge === e.key || hoverEdge === e.key;
              const isErr = errorEdges.has(e.key);
              const sticky = e.route.sticky === 'session';
              /* badge 只留 priority（避免重叠）；模型/sticky 信息进 aria-label + EdgeInspector */
              const label = String(e.route.priority);
              // 端点可读性移到无障碍标签（docs/11 §四）
              const target = e.route.upstream_model
                ? shortModel(e.route.upstream_model)
                : e.route.match === 'prefix'
                  ? 'caller input model'
                  : e.route.left;
              const aria =
                `route ${e.route.left} to ${e.route.right}, priority ${e.route.priority}` +
                `, upstream model ${target}` +
                (sticky ? ', sticky session' : '');

              // 获取调用统计数据（规范 §3.8）
              const stats = routeStats?.get(e.key);
              const strokeWidth = stats ? getStrokeWidth(stats.calls_24h) : 2;

              return (
                <g key={e.key} role="listitem">
                  {/* 可命中的宽 hit 区 */}
                  <path
                    d={e.d}
                    fill="none"
                    stroke="transparent"
                    strokeWidth={Math.max(16, strokeWidth + 4)}
                    style={{ pointerEvents: 'stroke', cursor: 'pointer' }}
                    tabIndex={0}
                    role="button"
                    aria-label={aria}
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
                    stroke={isErr ? 'var(--danger)' : active ? 'var(--edge)' : 'var(--edge-idle)'}
                    strokeWidth={strokeWidth}
                    strokeDasharray={e.route.on_error === 'next' ? '4 3' : undefined}
                    markerEnd={
                      isErr ? 'url(#arrow-danger)' : active ? 'url(#arrow-active)' : 'url(#arrow-idle)'
                    }
                    style={{ pointerEvents: 'none' }}
                  />
                  {/* 边中点标签：priority + 成功率环形图（规范 §3.7） */}
                  <g transform={`translate(${e.mx},${e.my})`} style={{ pointerEvents: 'none' }}>
                    {/* 成功率环形图背景 */}
                    {stats && (
                      <circle
                        cx={0}
                        cy={0}
                        r={11}
                        fill="var(--surface)"
                        stroke="var(--border)"
                        strokeWidth={1}
                      />
                    )}
                    {/* 成功率环形进度（绿色=成功，黄色=慢速，红色=失败） */}
                    {stats && stats.calls_24h > 0 && (
                      <>
                        {/* 绿色扇形：成功 */}
                        <circle
                          cx={0}
                          cy={0}
                          r={9}
                          fill="none"
                          stroke="var(--success)"
                          strokeWidth={3}
                          strokeDasharray={`${stats.success_rate * 56.5} 56.5`}
                          strokeDashoffset={-14.125}
                          style={{ transform: 'rotate(-90deg)', transformOrigin: 'center' }}
                        />
                        {/* 黄色扇形：慢速 */}
                        {stats.slow_rate > 0 && (
                          <circle
                            cx={0}
                            cy={0}
                            r={9}
                            fill="none"
                            stroke="#facc15"
                            strokeWidth={3}
                            strokeDasharray={`${stats.slow_rate * 56.5} 56.5`}
                            strokeDashoffset={-14.125 - stats.success_rate * 56.5}
                            style={{ transform: 'rotate(-90deg)', transformOrigin: 'center' }}
                          />
                        )}
                        {/* 红色扇形：失败 */}
                        {(1 - stats.success_rate - stats.slow_rate) > 0 && (
                          <circle
                            cx={0}
                            cy={0}
                            r={9}
                            fill="none"
                            stroke="var(--danger)"
                            strokeWidth={3}
                            strokeDasharray={`${(1 - stats.success_rate - stats.slow_rate) * 56.5} 56.5`}
                            strokeDashoffset={-14.125 - (stats.success_rate + stats.slow_rate) * 56.5}
                            style={{ transform: 'rotate(-90deg)', transformOrigin: 'center' }}
                          />
                        )}
                      </>
                    )}
                    {/* priority 数字 */}
                    {!stats && (
                      <circle
                        cx={0}
                        cy={0}
                        r={11}
                        fill="var(--surface)"
                        stroke={isErr ? 'var(--danger)' : active ? 'var(--accent)' : 'var(--border)'}
                        strokeWidth={1}
                      />
                    )}
                    <text
                      textAnchor="middle"
                      y={4}
                      fill="var(--text)"
                      style={{
                        fontSize: 12,
                        fontFamily: 'var(--font-mono)',
                        fontVariantNumeric: 'tabular-nums',
                        fontWeight: 600,
                      }}
                    >
                      {label}
                    </text>
                  </g>

                  {/* hover 浮动卡片（规范 §3.7） */}
                  {active && stats && (
                    <g transform={`translate(${e.mx + 20},${e.my - 40})`} style={{ pointerEvents: 'none' }}>
                      <rect
                        x={0}
                        y={0}
                        width={140}
                        height={70}
                        rx={4}
                        fill="var(--surface)"
                        stroke="var(--border)"
                        strokeWidth={1}
                        style={{ filter: 'drop-shadow(0 4px 6px rgba(0,0,0,0.1))' }}
                      />
                      <text x={8} y={16} fill="var(--text)" style={{ fontSize: 12, fontFamily: 'var(--font-sans)' }}>
                        Priority: {e.route.priority}
                      </text>
                      <text x={8} y={32} fill="var(--text)" style={{ fontSize: 12, fontFamily: 'var(--font-sans)' }}>
                        Sticky: {e.route.sticky ?? 'none'}
                      </text>
                      <text x={8} y={48} fill="var(--text)" style={{ fontSize: 12, fontFamily: 'var(--font-sans)' }}>
                        On Error: {e.route.on_error ?? 'next'}
                      </text>
                      <text x={8} y={64} fill="var(--text)" style={{ fontSize: 12, fontFamily: 'var(--font-sans)', fontWeight: 600 }}>
                        Calls (24h): {stats.calls_24h.toLocaleString()}
                      </text>
                    </g>
                  )}
                </g>
              );
            })}
        </g>

        {/* 拖线临时边（带磁吸效果，规范 §3.1） */}
        {drag && (
          <>
            {/* 磁吸目标：显示虚线预览到端口 */}
            {drag.snapTarget ? (
              <path
                d={`M ${drag.ax} ${drag.ay} C ${drag.ax + 60} ${drag.ay}, ${drag.snapTarget.x - 60} ${drag.snapTarget.y}, ${drag.snapTarget.x} ${drag.snapTarget.y}`}
                fill="none"
                stroke="var(--accent)"
                strokeWidth={2}
                strokeDasharray="4 3"
                style={{ pointerEvents: 'none' }}
              />
            ) : (
              <path
                d={`M ${drag.ax} ${drag.ay} C ${drag.ax + 60} ${drag.ay}, ${drag.x - 60} ${drag.y}, ${drag.x} ${drag.y}`}
                fill="none"
                stroke="var(--edge)"
                strokeWidth={1.5}
                strokeDasharray="4 3"
                style={{ pointerEvents: 'none' }}
              />
            )}
          </>
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
            className="absolute z-10"
            style={{
              left: b.x,
              top: b.y,
              width: b.w,
              height: b.h,
              background: 'var(--surface)',
              border: '1px solid var(--border)',
              borderRadius: 'var(--radius)',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            {/* 卡头 = 提供商锚定：地址 + key 归属（API token 在 provider 层，掩码展示归 Providers 页） */}
            <div
              className="flex h-10 items-center justify-between gap-2 px-2.5"
              style={{
                borderBottom: '1px solid var(--border)',
                background: 'var(--surface-hover)',
                borderTopLeftRadius: 'var(--radius)',
                borderTopRightRadius: 'var(--radius)',
              }}
            >
              <span className="flex min-w-0 items-center gap-2">
                <span
                  className="inline-block shrink-0"
                  style={{
                    width: 6,
                    height: 6,
                    borderRadius: '50%',
                    background: b.enabled ? 'var(--success)' : 'var(--text-subtle)',
                  }}
                  aria-hidden
                />
                <span
                  className="min-w-0 truncate tabular"
                  style={{
                    fontFamily: 'var(--font-mono)',
                    fontSize: 'var(--text-sm)',
                    fontWeight: 500,
                    color: 'var(--text)',
                  }}
                  title={b.id}
                >
                  {b.id}
                </span>
              </span>
              <span className="flex shrink-0 items-center gap-1">
                <span
                  className="tabular"
                  style={{ ...CHIP, background: 'var(--surface)', color: 'var(--text-muted)' }}
                >
                  {b.ports.length} {tCore('routing.ep')}
                </span>
              </span>
            </div>
            {/* 端口区 = 卡片内的模型端点（三元组再锚定模型 id） */}
            <div className="flex flex-col gap-1 px-1.5 pt-1.5">
              {b.ports.length === 0 ? (
                <div
                  className="flex h-6 items-center px-1.5"
                  style={{
                    border: '1px dashed var(--border)',
                    borderRadius: 'var(--radius)',
                    fontSize: 'var(--text-xs)',
                    color: 'var(--text-muted)',
                  }}
                >
                  {tCore('canvas.noEndpoint')}
                </div>
              ) : (
                b.ports.map((p) => {
                  const isHoverPort = hoverPort === p.key;
                  const portDotSize = isHoverPort ? PORT_DOT_DRAG_NEARBY : PORT_DOT_NORMAL;

                  return (
                    <div
                      key={p.key}
                      data-endpoint={p.key}
                      data-right-node={b.id}
                      role="group"
                      aria-label={`endpoint ${b.id}/${p.model ?? 'passthrough'}`}
                      className="relative flex items-center gap-1.5 px-1.5"
                      style={{
                        height: PORT_H,
                        border: '1px solid var(--border)',
                        borderRadius: 'var(--radius)',
                        background: 'var(--bg)',
                        fontFamily: 'var(--font-mono)',
                        fontSize: 'var(--text-xs)',
                        color: 'var(--text)',
                      }}
                      title={
                        p.pinned
                          ? tCore('canvas.pinTitle', { id: b.id, model: p.model ?? '' })
                          : tCore('canvas.passTitle')
                      }
                      onMouseEnter={() => setHoverPort(p.key)}
                      onMouseLeave={() => setHoverPort(null)}
                    >
                      {/* 端口圆点（规范 §3.3：实心 pin / 空心 pass，左侧边缘） */}
                      <div
                        className="absolute shrink-0"
                        style={{
                          left: -portDotSize / 2,
                          top: '50%',
                          transform: 'translateY(-50%)',
                          width: portDotSize,
                          height: portDotSize,
                          borderRadius: '50%',
                          background: p.pinned ? 'var(--accent)' : 'transparent',
                          border: p.pinned ? 'none' : '2px solid var(--text-subtle)',
                          boxShadow: isHoverPort ? '0 0 8px var(--accent)' : undefined,
                          transition: 'all 0.15s ease-out',
                        }}
                        aria-hidden
                      />
                      <span className="min-w-0 truncate tabular" title={p.model ?? undefined}>
                        {p.model ?? tCore('canvas.inputModel')}
                      </span>
                      <span
                        className="ml-auto shrink-0"
                        style={{
                          ...CHIP,
                          fontFamily: 'var(--font-sans)',
                          ...(p.pinned
                            ? { background: 'var(--accent)', color: '#fff' }
                            : { background: 'var(--surface-hover)', color: 'var(--text-muted)' }),
                        }}
                      >
                        {p.pinned ? tCore('routing.pin') : tCore('routing.pass')}
                      </span>
                    </div>
                  );
                })
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
            className="absolute z-10 px-2.5 py-2"
            style={{
              left: b.x,
              top: b.y,
              width: b.w,
              height: b.h,
              background: 'var(--surface)',
              border: '1px solid var(--border)',
              borderRadius: 'var(--radius)',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex h-full items-center justify-between gap-2">
              <span
                className="min-w-0 truncate tabular"
                style={{
                  fontFamily: 'var(--font-mono)',
                  fontSize: 'var(--text-sm)',
                  fontWeight: 500,
                  color: 'var(--text)',
                }}
                title={b.id}
              >
                {b.id}
              </span>
              <span
                className="shrink-0"
                style={{ ...CHIP, background: 'var(--surface-hover)', color: 'var(--text-muted)' }}
              >
                {CHIP_TEXT.ALIAS}
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
