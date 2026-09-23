import { useEffect, useMemo, useRef, useState } from 'react';
import {
  AdminApiError,
  edgeKey,
  findCyclicEdgeKeys,
  listProviders,
  listRoutes,
  normalizeRoute,
  putRoutes,
  type Route,
} from '../api/admin';
import {
  MODELS_FIXTURE,
  ROUTES_EXAMPLE_TOML,
  ROUTES_FIXTURE,
} from '../fixtures/routes.mock';
import {
  BipartiteCanvas,
  type CanvasProvider,
} from '../components/routing/BipartiteCanvas';
import { RouteTableForm } from '../components/routing/RouteTableForm';
import { useToast } from '../app/feedback';

const ser = (rs: Route[]) => JSON.stringify(rs);

/**
 * Routing 页（design/01 §6.2，契约 03）：
 * 二部图拖线/选边/浮层四字段 + RouteTableForm 键盘等价 +
 * 变更 debounce 400ms PUT（本地环检 + 400 标红）+ 删除乐观失败回滚 + UNSAVED。
 * mock-first：admin 适配层 DEV 默认 mock（H3 切真 API）。
 */
export function RoutingPage() {
  const [routes, setRoutes] = useState<Route[]>([]);
  const [baseline, setBaseline] = useState<Route[]>([]);
  const [providers, setProviders] = useState<CanvasProvider[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [errorEdges, setErrorEdges] = useState<ReadonlySet<string>>(new Set());
  const [lastError, setLastError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [showTable, setShowTable] = useState(false);
  const { toast } = useToast();

  const routesRef = useRef(routes);
  routesRef.current = routes;
  const baselineRef = useRef(baseline);
  baselineRef.current = baseline;
  const seqRef = useRef(0);

  const dirty = ser(routes) !== ser(baseline);

  const load = () => {
    setLoading(true);
    setLoadError(null);
    listRoutes()
      .then((r) => {
        const norm = r.routes.map(normalizeRoute);
        setRoutes(norm);
        setBaseline(norm.map((x) => ({ ...x })));
        setLoading(false);
      })
      .catch((e) => {
        setLoadError((e as Error).message);
        setLoading(false);
      });
    listProviders()
      .then((p) => setProviders(p.providers.map((x) => ({ id: x.id, enabled: x.enabled }))))
      .catch(() => setProviders([])); // 提供商灯失败不阻塞路由编辑
  };

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /* ---- 保存管线：本地环检 → debounce PUT → 成功记 baseline / 失败标红或回滚 ---- */
  const save = async (current: Route[]) => {
    // design/01 §6.2：PUT 前本地检环 — 不发请求、标红涉及边
    const localCycle = findCyclicEdgeKeys(current);
    if (localCycle.length > 0) {
      setErrorEdges(new Set(localCycle));
      setLastError('路由成环，已拒绝写入');
      return;
    }
    const seq = ++seqRef.current;
    setSaving(true);
    setLastError(null);
    setErrorEdges(new Set());
    try {
      const res = await putRoutes(current);
      if (seq !== seqRef.current) return;
      setBaseline(res.routes.map(normalizeRoute));
      // 不回写 routes：保留用户在途编辑（若并发新编辑会再次置脏并重排 debounce）
    } catch (e) {
      if (seq !== seqRef.current) return;
      const msg = e instanceof Error ? e.message : String(e);
      const cycle = e instanceof AdminApiError ? e.cycleEdges : undefined;
      if (cycle && cycle.length > 0) {
        // 服务端 400（环）：保留展示标红边 + UNSAVED，不回滚
        setErrorEdges(new Set(cycle));
        setLastError(msg);
      } else {
        // 其余失败（含删除失败）：回滚到 baseline
        setRoutes(baselineRef.current.map(normalizeRoute));
        setSelected(null);
        setLastError(msg);
        toast('danger', '保存失败，已回滚');
      }
    } finally {
      if (seq === seqRef.current) setSaving(false);
    }
  };

  /* debounce 400ms（design/01 §6.2）：仅在脏且非加载时排队 */
  useEffect(() => {
    if (loading || !dirty) return;
    const timer = window.setTimeout(() => void save(routesRef.current), 400);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [routes, baseline, loading, dirty]);

  /* ---- 选中边 + 键盘删除（Backspace / Delete） ---- */
  const selectedRoute = useMemo(
    () => routes.find((r) => edgeKey(r.left, r.right) === selected) ?? null,
    [routes, selected],
  );

  useEffect(() => {
    if (selected === null) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Backspace' && e.key !== 'Delete') return;
      const t = e.target as HTMLElement | null;
      if (
        t &&
        (t.tagName === 'INPUT' ||
          t.tagName === 'TEXTAREA' ||
          t.tagName === 'SELECT' ||
          t.isContentEditable)
      ) {
        return;
      }
      e.preventDefault();
      const idx = routesRef.current.findIndex(
        (r) => edgeKey(r.left, r.right) === selected,
      );
      if (idx >= 0) removeAt(idx);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected]);

  /* ---- 变更操作（全部乐观进 state，由 debounce effect 统一 PUT） ---- */

  const onCreateEdge = (left: string, right: string, model?: string | null) => {
    const key = edgeKey(left, right);
    const cur = routesRef.current;
    if (cur.some((r) => edgeKey(r.left, r.right) === key)) {
      setSelected(key);
      return;
    }
    const priors = cur.filter((r) => r.left === left).map((r) => r.priority);
    const route = normalizeRoute({
      left,
      right,
      match: left.includes('*') ? 'prefix' : 'exact',
      priority: priors.length > 0 ? Math.max(...priors) + 10 : 10,
      sticky: 'none',
      on_error: 'next',
      // 端口落点 = 钉死模型（透传端口 model=null / 卡片空白 undefined → 不写 = exact 同名钉死）
      ...(model ? { upstream_model: model } : {}),
    } as Route);
    setRoutes([...cur, route]);
    setErrorEdges(new Set());
    setSelected(edgeKey(route.left, route.right));
  };

  const patchAt = (index: number, patch: Partial<Route>) => {
    const cur = routesRef.current;
    const target = cur[index];
    if (!target) return;
    const oldKey = edgeKey(target.left, target.right);
    const next = normalizeRoute({ ...target, ...patch });
    setRoutes(cur.map((r, i) => (i === index ? next : r)));
    const newKey = edgeKey(next.left, next.right);
    if (selected === oldKey && newKey !== oldKey) setSelected(newKey);
  };

  const patchByKey = (key: string, patch: Partial<Route>) => {
    const idx = routesRef.current.findIndex((r) => edgeKey(r.left, r.right) === key);
    if (idx >= 0) patchAt(idx, patch);
  };

  const removeAt = (index: number) => {
    const cur = routesRef.current;
    const target = cur[index];
    if (!target) return;
    const key = edgeKey(target.left, target.right);
    setRoutes([...cur.slice(0, index), ...cur.slice(index + 1)]); // 乐观移除
    if (selected === key) setSelected(null);
    setErrorEdges((prev) => {
      if (!prev.has(key)) return prev;
      const next = new Set(prev);
      next.delete(key);
      return next;
    });
  };

  const removeByKey = (key: string) => {
    const idx = routesRef.current.findIndex((r) => edgeKey(r.left, r.right) === key);
    if (idx >= 0) removeAt(idx);
  };

  /* ---- RouteTableForm：加行（取首个未使用的 left×right 组合） ---- */
  const onAddRow = () => {
    const cur = routesRef.current;
    const leftPool = Array.from(
      new Set([...MODELS_FIXTURE.map((m) => m.id), ...cur.map((r) => r.left)]),
    );
    const rightPool = Array.from(
      new Set([...providers.map((p) => p.id), ...cur.map((r) => r.right)]),
    );
    for (const left of leftPool) {
      for (const right of rightPool) {
        if (left === right) continue;
        if (cur.some((r) => r.left === left && r.right === right)) continue;
        onCreateEdge(left, right);
        return;
      }
    }
    toast('warn', '没有可用的 left×right 组合');
  };

  const importExample = () => {
    setRoutes(ROUTES_FIXTURE.map(normalizeRoute));
    setErrorEdges(new Set());
    setLastError(null);
    toast('ok', '已导入 example routes（UNSAVED → 自动保存）');
  };

  /* ---- 左列：models ∪ routes.left ---- */
  const leftIds = useMemo(() => {
    const seen = new Set<string>();
    const out: string[] = [];
    for (const m of MODELS_FIXTURE) {
      if (!seen.has(m.id)) {
        seen.add(m.id);
        out.push(m.id);
      }
    }
    for (const r of routes) {
      if (!seen.has(r.left)) {
        seen.add(r.left);
        out.push(r.left);
      }
    }
    return out;
  }, [routes]);

  const statusBadge = saving ? (
    <span className="border border-ink bg-ink px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-widest text-bg animate-pulse">
      Saving…
    </span>
  ) : dirty ? (
    <span className="border border-warn px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-widest text-warn">
      UNSAVED
    </span>
  ) : (
    <span className="border border-border px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
      synced
    </span>
  );

  return (
    <div className="mx-auto max-w-7xl px-6 py-8">
      {/* 工具条 */}
      <div className="flex flex-wrap items-center justify-between gap-3 border border-border bg-panel px-4 py-2.5">
        <div className="flex items-center gap-3">
          <span className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
            Routing
          </span>
          <span className="font-mono text-xs text-inkMuted tabular">
            {routes.length} edges
          </span>
          <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
            model endpoint → model endpoint
          </span>
          <span aria-live="polite">{statusBadge}</span>
          {lastError && (
            <span role="status" className="font-mono text-xs text-danger">
              {lastError}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setShowTable((v) => !v)}
            aria-pressed={showTable}
            className={
              'h-8 px-2.5 font-mono text-xs transition-colors ' +
              (showTable
                ? 'border border-ink bg-ink text-bg'
                : 'border border-border bg-panel text-ink hover:border-ink')
            }
          >
            TABLE
          </button>
          <button
            type="button"
            onClick={load}
            className="h-8 border border-border bg-panel px-2.5 font-mono text-xs text-ink hover:border-ink"
          >
            刷新
          </button>
        </div>
      </div>

      {/* 主体 */}
      {loading ? (
        <div className="mt-4 h-96 animate-pulse border border-border bg-panel" aria-busy="true" />
      ) : loadError ? (
        <section className="mt-4 border border-danger bg-panel p-6" role="alert">
          <p className="text-sm text-danger">加载失败：{loadError}</p>
          <button
            type="button"
            onClick={load}
            className="mt-3 h-8 border border-border bg-panel px-3 font-mono text-xs text-ink hover:border-ink"
          >
            重试
          </button>
        </section>
      ) : routes.length === 0 ? (
        /* 空态（design/01 §6.2） */
        <section className="mt-4 border border-border bg-panel p-8 text-center">
          <p className="text-sm text-inkMuted">还没有路由。</p>
          <pre className="mx-auto mt-3 max-w-lg overflow-x-auto border border-border bg-bg p-3 text-left font-mono text-xs leading-relaxed text-ink">
            {ROUTES_EXAMPLE_TOML}
          </pre>
          <button
            type="button"
            onClick={importExample}
            className="mt-4 h-8 border border-ink bg-ink px-3 font-mono text-xs text-bg hover:bg-bg hover:text-ink"
          >
            一键导入 example
          </button>
        </section>
      ) : (
        <>
          {/* 二部图画布 + 边浮层 */}
          <div className="mt-4 border border-border">
            <BipartiteCanvas
              routes={routes}
              leftIds={leftIds}
              providers={providers}
              errorEdges={errorEdges}
              selectedEdge={selected}
              selectedRoute={selectedRoute}
              onSelectEdge={setSelected}
              onCreateEdge={onCreateEdge}
              onPatchEdge={patchByKey}
              onDeleteEdge={removeByKey}
            />
          </div>

          {/* 键盘等价路由表 */}
          {showTable && (
            <div className="mt-4">
              <RouteTableForm
                routes={routes}
                errorEdges={errorEdges}
                onPatchAt={patchAt}
                onDeleteAt={removeAt}
                onAdd={onAddRow}
              />
            </div>
          )}
        </>
      )}
    </div>
  );
}
