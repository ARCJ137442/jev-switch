import { useEffect, useState } from 'react';
import { Boxes, FlaskConical, GitBranch, Network, Plus, RefreshCw, Server, Settings, type LucideIcon } from 'lucide-react';
import {
  AdminApiError,
  getStatus,
  listProviders,
  listRoutes,
  probeProvider,
  type AdminProvider,
  type AdminStatus,
  type Route,
} from '../api/admin';
import { useServerStatus } from '../app/Shell';
import { useI18n, type MessageKey } from '../i18n';
import { InstanceSettings } from '../components/settings/InstanceSettings';
import { AccessDashboard } from '../components/access/AccessDashboard';
import { useAuth } from '../auth/AuthContext';
import { RuntimeTelemetry } from '../components/dashboard/RuntimeTelemetry';
import { probeProvidersConcurrently, type ProviderHealth } from './dashboardHealth';

/**
 * Dashboard（v2.0 · 设计稿 docs/design/UI-REDESIGN-v2.md §2）
 *
 * 取代旧 HomePage 的「配置页」定位，改为控制台仪表盘：
 * ① 状态速览 —— daemon / mode+bind / providers 可达性 / routes 摘要，图形优先
 * ② 就地操作 —— 实例设置在本页折叠展开；探测仅表示可达性
 * ③ 活动流水 —— 最近请求记录与调用状态（AccessDashboard）
 *
 * 数据源均为真实接口：getStatus / listProviders / listRoutes / probeProvider。
 */

const REACHABILITY_COLOR: Record<ProviderHealth['state'], string> = {
  reachable: 'var(--success)',
  unreachable: 'var(--danger)',
  disabled: 'var(--text-subtle)',
  unknown: 'var(--text-subtle)',
};

function SummaryMark({ icon: Icon }: { icon: LucideIcon }) {
  return (
    <span
      className="inline-grid h-8 w-8 shrink-0 place-items-center"
      style={{ color: 'var(--accent)', background: 'color-mix(in srgb, var(--accent) 12%, transparent)', borderRadius: 'var(--radius)' }}
      aria-hidden="true"
    >
      <Icon size={17} strokeWidth={1.8} />
    </span>
  );
}

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
  const auth = useAuth();
  const server = useServerStatus();

  const [status, setStatus] = useState<AdminStatus | null>(null);
  const [providers, setProviders] = useState<AdminProvider[] | null>(null);
  const [routes, setRoutes] = useState<Route[] | null>(null);
  const [health, setHealth] = useState<Record<string, ProviderHealth>>({});
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [loadError, setLoadError] = useState<'auth' | 'failed' | null>(null);
  const [refresh, setRefresh] = useState(0);
  const copy = (key: string, vars?: Record<string, string | number>) => t(key as MessageKey, vars);

  /* 初始加载 —— 首次挂载时拉取所有数据 */
  useEffect(() => {
    setStatus(null);
    setProviders(null);
    setRoutes(null);
    setHealth({});
    setLoadError(null);
    if (auth.isReadOnly) {
      return;
    }
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
          setProviders(null);
          setRoutes(null);
          setLoadError(err instanceof AdminApiError && (err.status === 401 || err.status === 403) ? 'auth' : 'failed');
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [auth.identity, auth.isReadOnly, refresh]);

  /* 健康自动探测 —— 不再要求用户手点 Probe（审查报告 §功能盲区） */
  useEffect(() => {
    if (!providers || providers.length === 0) return;
    let cancelled = false;
    let inFlight = false;
    let timer: number | undefined;
    const run = async () => {
      if (cancelled || inFlight) return;
      inFlight = true;
      try {
        const nextHealth = await probeProvidersConcurrently(providers, probeProvider);
        if (!cancelled) setHealth(nextHealth);
      } finally {
        inFlight = false;
        if (!cancelled) timer = window.setTimeout(() => void run(), 30000);
      }
    };
    void run();
    return () => {
      cancelled = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [providers]);

  const daemonUp = server.status === 'ok';
  const totalCount = providers?.length ?? 0;
  const reachableCount = Object.values(health).filter((h) => h.state === 'reachable').length;
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

  if (auth.isReadOnly) {
    return (
      <div className="page-container dashboard-container mx-auto">
        <h1 className="mb-6 font-semibold" style={{ fontSize: 'var(--text-2xl)' }}>
          {t('shell.navDashboard')}
        </h1>
        <AccessDashboard />
      </div>
    );
  }

  return (
    <div className="page-container dashboard-container mx-auto">
      <h1 className="mb-6 font-semibold" style={{ fontSize: 'var(--text-2xl)' }}>
        {t('shell.navDashboard')}
      </h1>

      {loadError && (
        <div role="alert" className="mb-4 flex flex-wrap items-center justify-between gap-3 p-3" style={{ ...card, color: 'var(--text-muted)' }}>
          <span>{copy(loadError === 'auth' ? 'overview.authRequired' : 'overview.loadFailed')}</span>
          <button type="button" style={btn} onClick={() => setRefresh((value) => value + 1)}><RefreshCw size={14} className="mr-1 inline-block align-[-2px]" aria-hidden="true" />{copy('overview.retry')}</button>
        </div>
      )}

      {/* ① 状态速览 */}
      <div className="mb-5 grid gap-3 sm:mb-6 sm:gap-4 md:grid-cols-2 xl:grid-cols-4">
        {/* daemon */}
        <section className="fade-in p-4 sm:p-5" style={card}>
          <div className="mb-2 flex items-center gap-2.5">
            <SummaryMark icon={Server} />
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

        {/* mode + bind：设置在 Dashboard 内部展开，避免跳出当前概览 */}
        <section className="fade-in p-4 sm:p-5" style={card}>
          <div className="mb-2 flex items-center justify-between gap-2">
            <span className="flex min-w-0 items-center gap-2 font-semibold capitalize" style={{ fontSize: 'var(--text-lg)' }}>
              <SummaryMark icon={Network} />
              {status?.mode ? t('dash.modeLabel', { mode: status.mode }) : '—'}
            </span>
            <button type="button" onClick={() => setSettingsOpen(true)} style={{ ...btn, color: 'var(--accent)', padding: '0.3rem 0.6rem' }}>
              <Settings size={14} className="mr-1 inline-block align-[-2px]" aria-hidden="true" />
              {copy('instance.manage')}
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

        {/* provider 连通性探测（不是推理健康检查） */}
        <a href="#/providers" className="fade-in card-hover block p-4 sm:p-5" style={card}>
          <div className="mb-2 flex items-center gap-2">
            <span className="flex items-center gap-1" aria-hidden>
              {(providers ?? []).slice(0, 4).map((p) => {
                const st = health[p.id]?.state ?? 'unknown';
                return (
                  <span
                    key={p.id}
                    style={{
                      width: 8,
                      height: 8,
                      borderRadius: '50%',
                      background: REACHABILITY_COLOR[st],
                      display: 'inline-block',
                    }}
                  />
                );
              })}
            </span>
            <span className="flex items-center gap-2 font-semibold" style={{ fontSize: 'var(--text-lg)' }}>
              <SummaryMark icon={Boxes} />
              {providers === null ? '—' : copy('overview.reachableCount', { reachable: reachableCount, total: totalCount })}
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
                      background: REACHABILITY_COLOR[h?.state ?? 'unknown'],
                      display: 'inline-block',
                      flexShrink: 0,
                    }}
                    aria-hidden
                  />
                  <span style={{ fontFamily: 'var(--font-mono)' }}>{p.id}</span>
                  {h?.ms !== null && h?.ms !== undefined && (
                    <span className="tabular" style={{ color: 'var(--text-subtle)' }}>
                      {copy('overview.probeLatency', { ms: h.ms })}
                    </span>
                  )}
                  <span style={{ color: 'var(--text-subtle)' }}>
                    {copy(`overview.${h?.state ?? 'unknown'}`)}
                  </span>
                </div>
              );
            })}
            {providers?.length === 0 && <div style={cardLabel}>{t('dash.noProviders')}</div>}
          </div>
        </a>

        {/* routes 摘要 */}
        <a href="#/routing" className="fade-in card-hover block p-4 sm:p-5" style={card}>
          <div className="mb-2 flex items-center gap-2 font-semibold" style={{ fontSize: 'var(--text-lg)' }}>
            <SummaryMark icon={GitBranch} />
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

      <InstanceSettings
        status={status}
        onStatusChange={setStatus}
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
      />

      <RuntimeTelemetry />

      {/* ② Token 管理、调用归属与真实活动数据仍留在 Dashboard 内，不增加主导航。 */}
      <AccessDashboard />

      {/* ③ 快捷操作 */}
      <div className="fade-in flex flex-wrap gap-3">
        <a
          href="#/playground"
          className="inline-flex items-center gap-2"
          style={{
            ...btn,
            background: 'var(--accent)',
            borderColor: 'var(--accent)',
            color: '#fff',
            fontWeight: 600,
          }}
        >
          <FlaskConical size={16} aria-hidden="true" />
          {t('dash.testJev')}
        </a>
        <a href="#/providers" className="inline-flex items-center gap-2" style={btn}>
          <Plus size={16} aria-hidden="true" />
          {t('dash.addProvider')}
        </a>
        <a href="#/routing" className="inline-flex items-center gap-2" style={btn}>
          <GitBranch size={16} aria-hidden="true" />
          {t('dash.editRoutesAction')}
        </a>
      </div>
    </div>
  );
}
