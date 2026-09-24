import { useEffect, useState } from 'react';
import {
  getStatus,
  listProviders,
  listRoutes,
  probeProvider,
  putMode,
  type AdminProvider,
  type AdminStatus,
  type Route,
} from '../api/admin';
import { useServerStatus } from '../app/Shell';
import { useToast } from '../app/feedback';
import { useI18n } from '../i18n';

/**
 * Dashboard（v2.0 · 设计稿 docs/design/UI-REDESIGN-v2.md §2）
 *
 * 取代旧 HomePage 的「配置页」定位，改为控制台仪表盘：
 * ① 状态速览 —— daemon / mode+bind / providers 健康 / routes 摘要，图形优先
 * ② 就地操作 —— mode 一键切（无二次确认框）、健康自动探测
 * ③ 活动流水 —— 最近请求（后端 /v1/admin/logs 未落地前为占位空态）
 *
 * 数据源均为真实接口：getStatus / listProviders / listRoutes / probeProvider。
 */

type Health = 'ok' | 'slow' | 'fail' | 'off' | 'unknown';

const HEALTH_COLOR: Record<Health, string> = {
  ok: 'var(--success)',
  slow: 'var(--warning)',
  fail: 'var(--danger)',
  off: 'var(--text-subtle)',
  unknown: 'var(--text-subtle)',
};

function fmtUptime(sec: number): string {
  const d = Math.floor(sec / 86400);
  const h = Math.floor((sec % 86400) / 3600);
  const m = Math.floor((sec % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

export function DashboardPage() {
  const { t } = useI18n();
  const { toast } = useToast();
  const server = useServerStatus();

  const [status, setStatus] = useState<AdminStatus | null>(null);
  const [providers, setProviders] = useState<AdminProvider[] | null>(null);
  const [routes, setRoutes] = useState<Route[] | null>(null);
  const [health, setHealth] = useState<Record<string, { state: Health; ms: number | null }>>({});
  const [switching, setSwitching] = useState(false);

  /* 初始加载 —— 首次挂载时拉取所有数据 */
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [st, prov, rt] = await Promise.all([
          getStatus(),
          listProviders(),
          listRoutes(),
        ]);
        if (!cancelled) {
          setStatus(st.status);
          setProviders(prov.providers);
          setRoutes(rt.routes);
        }
      } catch (err) {
        if (!cancelled) {
          setStatus(null);
          setProviders([]);
          setRoutes([]);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  /* 健康自动探测 —— 不再要求用户手点 Probe（审查报告 §功能盲区） */
  useEffect(() => {
    if (!providers || providers.length === 0) return;
    let cancelled = false;
    const run = async () => {
      for (const p of providers) {
        if (cancelled) return;
        if (!p.enabled) {
          setHealth((h) => ({ ...h, [p.id]: { state: 'off', ms: null } }));
          continue;
        }
        try {
          const r = await probeProvider(p.id);
          if (cancelled) return;
          setHealth((h) => ({
            ...h,
            [p.id]: {
              state: !r.ok ? 'fail' : r.latency_ms >= 1000 ? 'slow' : 'ok',
              ms: r.latency_ms,
            },
          }));
        } catch {
          if (!cancelled) setHealth((h) => ({ ...h, [p.id]: { state: 'fail', ms: null } }));
        }
      }
    };
    void run();
    const timer = window.setInterval(run, 30000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [providers]);

  /* mode 一键切：无二次确认框（设计原则 P1·就地操作） */
  const switchMode = async () => {
    if (!status || switching) return;
    const next = status.mode === 'local' ? 'cloud' : 'local';
    setSwitching(true);
    try {
      const res = await putMode({ mode: next, admin_password: null });
      const rb = res.rebind;
      toast(
        'ok',
        'skipped' in rb
          ? `mode → ${res.mode} · rebind skipped: ${rb.skipped}`
          : `mode → ${res.mode} · ${rb.from} → ${rb.to}`,
      );
      const st = await getStatus();
      setStatus(st.status);
    } catch (e) {
      toast('danger', (e as Error).message);
    } finally {
      setSwitching(false);
    }
  };

  const daemonUp = server.status === 'ok';
  const totalCount = providers?.length ?? 0;
  const okCount = Object.values(health).filter((h) => h.state === 'ok').length;
  const prefixCount = routes?.filter((r) => r.match === 'prefix').length ?? 0;
  const exactCount = routes?.filter((r) => r.match === 'exact').length ?? 0;

  const card: React.CSSProperties = {
    background: 'var(--surface)',
    border: '1px solid var(--border)',
    borderRadius: 'var(--radius)',
  };
  const cardLabel: React.CSSProperties = {
    fontSize: 'var(--text-sm)',
    color: 'var(--text-muted)',
  };
  const btn: React.CSSProperties = {
    fontSize: 'var(--text-sm)',
    background: 'var(--surface-hover)',
    border: '1px solid var(--border)',
    borderRadius: 'var(--radius)',
    color: 'var(--text)',
    padding: '0.5rem 0.875rem',
  };

  return (
    <div className="mx-auto max-w-7xl px-6 py-8">
      <h1 className="mb-6 font-semibold" style={{ fontSize: 'var(--text-2xl)' }}>
        {t('shell.navDashboard')}
      </h1>

      {/* ① 状态速览 */}
      <div className="mb-6 grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {/* daemon */}
        <section className="fade-in p-5" style={card}>
          <div className="mb-2 flex items-center gap-2.5">
            <span
              className={daemonUp ? '' : 'status-danger'}
              style={{
                width: 10,
                height: 10,
                borderRadius: '50%',
                background: daemonUp ? 'var(--success)' : 'var(--danger)',
                display: 'inline-block',
              }}
              aria-hidden
            />
            <span className="font-semibold" style={{ fontSize: 'var(--text-lg)' }}>
              {daemonUp ? t('dash.running') : t('dash.unreachable')}
            </span>
          </div>
          <div style={cardLabel} className="tabular">
            {status ? `${t('dash.uptimePrefix')} ${fmtUptime(status.uptime_s)} · v${status.version}` : '—'}
          </div>
        </section>

        {/* mode + bind：一键切 */}
        <section className="fade-in p-5" style={card}>
          <div className="mb-2 flex items-center justify-between gap-2">
            <span className="font-semibold capitalize" style={{ fontSize: 'var(--text-lg)' }}>
              {status?.mode ?? '—'} mode
            </span>
            <button
              type="button"
              onClick={() => void switchMode()}
              disabled={switching || status === null}
              style={{ ...btn, color: 'var(--accent)', padding: '0.3rem 0.6rem' }}
              title={
                status
                  ? t('dash.switchTo', { mode: status.mode === 'local' ? 'cloud' : 'local' })
                  : undefined
              }
            >
              {switching ? t('dash.switching') : t('dash.switchBtn')}
            </button>
          </div>
          <div style={{ ...cardLabel, fontFamily: 'var(--font-mono)' }} className="tabular">
            {status?.bind ?? '—'}
          </div>
          {status && !status.password_set && status.mode === 'cloud' && (
            <div className="mt-2" style={{ fontSize: 'var(--text-xs)', color: 'var(--danger)' }}>
              {t('dash.pwUnsetWarn')}
            </div>
          )}
        </section>

        {/* providers 健康 */}
        <a href="#/providers" className="fade-in card-hover block p-5" style={card}>
          <div className="mb-2 flex items-center gap-2">
            <span className="flex items-center gap-1" aria-hidden>
              {(providers ?? []).slice(0, 4).map((p) => {
                const st = health[p.id]?.state ?? 'unknown';
                return (
                  <span
                    key={p.id}
                    className={st === 'slow' || st === 'fail' ? 'status-warning' : ''}
                    style={{
                      width: 8,
                      height: 8,
                      borderRadius: '50%',
                      background: HEALTH_COLOR[st],
                      display: 'inline-block',
                    }}
                  />
                );
              })}
            </span>
            <span className="font-semibold" style={{ fontSize: 'var(--text-lg)' }}>
              {providers === null ? '—' : t('dash.healthy', { ok: okCount, total: totalCount })}
            </span>
          </div>
          <div className="space-y-1">
            {(providers ?? []).slice(0, 3).map((p) => {
              const h = health[p.id];
              return (
                <div key={p.id} className="flex items-center gap-2" style={cardLabel}>
                  <span
                    style={{
                      width: 6,
                      height: 6,
                      borderRadius: '50%',
                      background: HEALTH_COLOR[h?.state ?? 'unknown'],
                      display: 'inline-block',
                      flexShrink: 0,
                    }}
                    aria-hidden
                  />
                  <span style={{ fontFamily: 'var(--font-mono)' }}>{p.id}</span>
                  {h?.ms !== null && h?.ms !== undefined && (
                    <span className="tabular" style={{ color: 'var(--text-subtle)' }}>
                      {h.ms}ms
                    </span>
                  )}
                </div>
              );
            })}
            {providers?.length === 0 && <div style={cardLabel}>{t('dash.noProviders')}</div>}
          </div>
        </a>

        {/* routes 摘要 */}
        <a href="#/routing" className="fade-in card-hover block p-5" style={card}>
          <div className="mb-2 font-semibold" style={{ fontSize: 'var(--text-lg)' }}>
            {routes === null ? '—' : t('dash.routes', { n: routes.length })}
          </div>
          <div style={cardLabel} className="tabular">
            {routes === null ? '—' : t('dash.routesSummary', { exact: exactCount, prefix: prefixCount })}
          </div>
          <div className="mt-2" style={{ fontSize: 'var(--text-sm)', color: 'var(--accent)' }}>
            {t('dash.editRoutes')}
          </div>
        </a>
      </div>

      {/* ② 活动流水（后端 /v1/admin/logs 未落地 → 诚实空态，不塞假数据） */}
      <section className="fade-in mb-6 p-5" style={card}>
        <div className="mb-3 flex items-baseline justify-between">
          <h2 className="font-semibold" style={{ fontSize: 'var(--text-xl)' }}>
            {t('dash.recentActivity')}
          </h2>
          <span style={{ fontSize: 'var(--text-xs)', color: 'var(--text-subtle)' }}>
            {t('dash.needsApi')}
          </span>
        </div>
        <div
          className="flex flex-col items-center justify-center gap-2 py-10 text-center"
          style={{ color: 'var(--text-muted)', fontSize: 'var(--text-sm)' }}
        >
          <span>{t('dash.historyUnavailable')}</span>
          <a href="#/playground" style={{ color: 'var(--accent)' }}>
            {t('dash.runTestRequest')}
          </a>
        </div>
      </section>

      {/* ③ 快捷操作 */}
      <div className="fade-in flex flex-wrap gap-3">
        <a
          href="#/playground"
          style={{
            ...btn,
            background: 'var(--accent)',
            borderColor: 'var(--accent)',
            color: '#fff',
            fontWeight: 600,
          }}
        >
          {t('dash.testJev')}
        </a>
        <a href="#/providers" style={btn}>
          {t('dash.addProvider')}
        </a>
        <a href="#/routing" style={btn}>
          {t('dash.editRoutesAction')}
        </a>
      </div>
    </div>
  );
}
