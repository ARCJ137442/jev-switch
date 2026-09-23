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
import { useI18n } from '../i18n';

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
  /* 空态示例 TOML 默认折叠（渐进披露，不一上来糊一屏代码） */
  const [showExample, setShowExample] = useState(false);
  const { toast } = useToast();
  const { t } = useI18n();

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
      setLastError(t('routing.cycle'));
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
        toast('danger', t('routing.saveFailed'));
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
    toast('warn', t('routing.noPair'));
  };

  const importExample = () => {
    setRoutes(ROUTES_FIXTURE.map(normalizeRoute));
    setErrorEdges(new Set());
    setLastError(null);
    toast('ok', t('routing.imported'));
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

  /* 状态胶囊（v2）：Saving=主色脉动 · UNSAVED=琥珀 · synced=中性 */
  const badgeBase: React.CSSProperties = {
    display: 'inline-flex',
    alignItems: 'center',
    gap: '0.375rem',
    whiteSpace: 'nowrap',
    borderRadius: 999,
    padding: '0.125rem 0.5rem',
    fontSize: 'var(--text-xs)',
    fontWeight: 500,
  };
  const statusBadge = saving ? (
    <span
      className="status-warning"
      style={{ ...badgeBase, background: 'var(--accent)', color: '#fff' }}
    >
      Saving…
    </span>
  ) : dirty ? (
    <span style={{ ...badgeBase, background: 'var(--warning-bg)', color: 'var(--warning)' }}>
      Unsaved
    </span>
  ) : (
    <span
      style={{ ...badgeBase, background: 'var(--surface-hover)', color: 'var(--text-muted)' }}
    >
      Synced
    </span>
  );

  const card: React.CSSProperties = {
    background: 'var(--surface)',
    border: '1px solid var(--border)',
    borderRadius: 'var(--radius)',
  };
  const btn: React.CSSProperties = {
    fontSize: 'var(--text-sm)',
    background: 'var(--surface-hover)',
    border: '1px solid var(--border)',
    borderRadius: 'var(--radius)',
    color: 'var(--text)',
    padding: '0.375rem 0.75rem',
  };
  const btnPrimary: React.CSSProperties = {
    ...btn,
    background: 'var(--accent)',
    borderColor: 'var(--accent)',
    color: '#fff',
    fontWeight: 600,
  };

  return (
    <div className="mx-auto max-w-7xl px-6 py-8">
      <h1 className="mb-6 font-semibold" style={{ fontSize: 'var(--text-2xl)' }}>
        {t('shell.navRouting')}
      </h1>

      {/* 工具条 */}
      <div className="flex flex-wrap items-center justify-between gap-3 px-4 py-2.5" style={card}>
        <div className="flex items-center gap-3">
          <span
            className="tabular"
            style={{ fontSize: 'var(--text-sm)', color: 'var(--text-muted)' }}
          >
            {t('routing.edges', { n: routes.length })}
          </span>
          <span aria-live="polite">{statusBadge}</span>
          {lastError && (
            <span
              role="status"
              style={{ fontSize: 'var(--text-sm)', color: 'var(--danger)' }}
            >
              {lastError}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setShowTable((v) => !v)}
            aria-pressed={showTable}
            style={showTable ? btnPrimary : btn}
            title={t('routing.tableTitle')}
          >
            {t('routing.table')}
          </button>
          <button type="button" onClick={load} style={btn}>
            {t('common.refresh')}
          </button>
        </div>
      </div>

      {/* 主体 */}
      {loading ? (
        <div
          className="mt-4 h-96 animate-pulse"
          style={card}
          aria-busy="true"
        />
      ) : loadError ? (
        <section
          className="mt-4 p-6"
          style={{
            ...card,
            borderColor: 'var(--danger)',
            background: 'var(--danger-bg)',
          }}
          role="alert"
        >
          <p style={{ fontSize: 'var(--text-sm)', color: 'var(--danger)' }}>
            {t('common.loadFailed')}
            {loadError}
          </p>
          <button type="button" onClick={load} style={{ ...btn, marginTop: '0.75rem' }}>
            {t('common.retry')}
          </button>
        </section>
      ) : routes.length === 0 ? (
        /* 空态：一键导入优先，示例 TOML 折叠（渐进披露） */
        <section className="fade-in mt-4 p-8 text-center" style={card}>
          <p style={{ fontSize: 'var(--text-base)', color: 'var(--text-muted)' }}>
            {t('routing.empty')}
          </p>
          <div className="mt-4 flex flex-wrap items-center justify-center gap-2">
            <button type="button" onClick={importExample} style={btnPrimary}>
              {t('routing.importExample')}
            </button>
            <button
              type="button"
              onClick={() => setShowExample((v) => !v)}
              aria-expanded={showExample}
              style={{ ...btn, background: 'transparent', borderColor: 'transparent', color: 'var(--accent)' }}
            >
              {showExample ? t('routing.hideExample') : t('routing.viewExample')}
            </button>
          </div>
          {showExample && (
            <pre
              className="fade-in mx-auto mt-4 max-w-lg overflow-x-auto p-3 text-left leading-relaxed"
              style={{
                border: '1px solid var(--border)',
                borderRadius: 'var(--radius)',
                background: 'var(--surface-hover)',
                fontFamily: 'var(--font-mono)',
                fontSize: 'var(--text-xs)',
                color: 'var(--text)',
              }}
            >
              {ROUTES_EXAMPLE_TOML}
            </pre>
          )}
        </section>
      ) : (
        <>
          {/* 二部图画布 + 边浮层 */}
          <div
            className="mt-4 overflow-hidden"
            style={{
              border: '1px solid var(--border)',
              borderRadius: 'var(--radius)',
            }}
          >
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
