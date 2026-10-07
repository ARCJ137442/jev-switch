import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Activity, ArrowRight, Boxes, FlaskConical, GitBranch, Plus, RefreshCw, Server, Settings, type LucideIcon } from 'lucide-react';
import { getBase } from '../api';
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
import { useStatusBarItems } from '../app/statusBar';
import { useI18n, type MessageKey } from '../i18n';
import { AccessDashboard } from '../components/access/AccessDashboard';
import { useAuth } from '../auth/AuthContext';
import { RuntimeTelemetry } from '../components/dashboard/RuntimeTelemetry';
import { probeProvidersConcurrently, type ProviderHealth } from './dashboardHealth';
import { PROVIDER_AUTO_PROBE_PREFIX, readAutoProviderProbe, readProviderAutoProbe, SETTINGS_CHANGE_EVENT, toggleFromStorage } from '../settings/preferences';
import { GatewayServiceControl } from '../components/dashboard/GatewayServiceControl';
import { DashboardLayoutEditor } from '../components/dashboard/DashboardLayoutEditor';
import { readDashboardLayout, type DashboardLayoutDocument } from '../components/dashboard/layout';
import { SectionHeader } from '../components/ui/SectionHeader';

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
  const [autoProbe, setAutoProbe] = useState(readAutoProviderProbe);
  const [probePreferenceRevision, setProbePreferenceRevision] = useState(0);
  const [loadError, setLoadError] = useState<'auth' | 'failed' | null>(null);
  const [refresh, setRefresh] = useState(0);
  const [activeRequests, setActiveRequests] = useState<number | null>(null);
  const [gatewayRunning, setGatewayRunning] = useState<boolean | null>(null);
  const [dashboardLayout, setDashboardLayout] = useState<DashboardLayoutDocument>(readDashboardLayout);
  const previousGatewayState = useRef<boolean | null>(null);
  const handleGatewayStateChange = useCallback((running: boolean | null) => {
    if (running === true && previousGatewayState.current !== true) setRefresh((value) => value + 1);
    previousGatewayState.current = running;
    setGatewayRunning(running);
  }, []);
  const copy = (key: string, vars?: Record<string, string | number>) => t(key as MessageKey, vars);
  const layoutItem = (componentId: string) => dashboardLayout.items.find((item) => item.componentId === componentId);
  const layoutStyle = (componentId: string): React.CSSProperties => {
    const item = layoutItem(componentId);
    const gridColumn = item?.width === 'full' ? '1 / -1' : item?.width === 'wide' || item?.width === 'half' ? 'span 2' : 'span 1';
    return { order: item?.order ?? 0, display: item?.visible === false ? 'none' : undefined, gridColumn };
  };

  useEffect(() => {
    const update = () => {
      setAutoProbe(readAutoProviderProbe());
      setProbePreferenceRevision((value) => value + 1);
    };
    const onStorage = (event: StorageEvent) => {
      const enabled = toggleFromStorage(event);
      if (enabled !== null) setAutoProbe(enabled);
      if (event.key?.startsWith(PROVIDER_AUTO_PROBE_PREFIX)) setProbePreferenceRevision((value) => value + 1);
    };
    window.addEventListener(SETTINGS_CHANGE_EVENT, update);
    window.addEventListener('storage', onStorage);
    return () => {
      window.removeEventListener(SETTINGS_CHANGE_EVENT, update);
      window.removeEventListener('storage', onStorage);
    };
  }, []);

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
    if (!autoProbe || !providers || providers.length === 0) return;
    let cancelled = false;
    let inFlight = false;
    let timer: number | undefined;
    const run = async () => {
      if (cancelled || inFlight) return;
      inFlight = true;
      try {
        const nextHealth = await probeProvidersConcurrently(
          providers.filter((provider) => readProviderAutoProbe(provider.id)),
          probeProvider,
        );
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
  }, [autoProbe, probePreferenceRevision, providers]);

  const daemonUp = server.status === 'ok';
  const runtimeUp = gatewayRunning ?? daemonUp;
  const apiAddress = (getBase() || window.location.origin).replace(/^https?:\/\//, '');
  const runtimeMode = status?.mode ? t('dash.modeLabel', { mode: status.mode }) : gatewayRunning === false ? t('dash.modeLabel', { mode: 'local' }) : '—';
  const totalCount = providers?.length ?? 0;
  const reachableCount = Object.values(health).filter((h) => h.state === 'reachable').length;
  const prefixCount = routes?.filter((r) => r.match === 'prefix').length ?? 0;
  const exactCount = routes?.filter((r) => r.match === 'exact').length ?? 0;
  const statusItems = useMemo(() => [
    { id: 'dashboard-active', label: `${t('telemetry.active')}: ${activeRequests ?? '—'}` },
    { id: 'dashboard-health', label: providers === null ? '—' : copy('overview.reachableCount', { reachable: reachableCount, total: totalCount }), tone: providers !== null && reachableCount === totalCount ? 'good' as const : 'default' as const },
  ], [activeRequests, providers, reachableCount, t, totalCount]);
  useStatusBarItems(statusItems);

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
        <SectionHeader title={t('shell.navDashboard')} />
        <AccessDashboard />
      </div>
    );
  }

  return (
    <div className="page-container dashboard-container mx-auto">
      <SectionHeader title={t('shell.navDashboard')} aside={<DashboardLayoutEditor layout={dashboardLayout} onChange={setDashboardLayout} />} />

      {loadError && (
        <div role="alert" className="mb-4 flex flex-wrap items-center justify-between gap-3 p-3" style={{ ...card, color: 'var(--text-muted)' }}>
          <span>{copy(loadError === 'auth' ? 'overview.authRequired' : 'overview.loadFailed')}</span>
          <button type="button" style={btn} onClick={() => setRefresh((value) => value + 1)}><RefreshCw size={14} className="mr-1 inline-block align-[-2px]" aria-hidden="true" />{copy('overview.retry')}</button>
        </div>
      )}

      {/* ① 状态速览 */}
      <div className="dashboard-layout-grid mb-5 sm:mb-6 md:mb-6">
        {/* runtime + address */}
        <section className="fade-in p-4 sm:p-5" style={{ ...card, ...layoutStyle('runtime') }}>
          <div className="mb-2 flex items-center gap-2.5">
            <SummaryMark icon={Server} />
            <span
              style={{
                width: 10,
                height: 10,
                borderRadius: '50%',
                background: runtimeUp ? 'var(--success)' : gatewayRunning === false ? 'var(--text-subtle)' : 'var(--danger)',
                display: 'inline-block',
              }}
              aria-hidden
            />
            <span className="min-w-0 font-semibold" style={{ fontSize: 'var(--text-lg)', color: runtimeUp ? 'var(--text)' : gatewayRunning === false ? 'var(--text-muted)' : 'var(--danger)' }}>
              {t('dash.runtimeMode', { state: runtimeUp ? t('dash.running') : gatewayRunning === false ? t('dash.stopped') : t('dash.unreachable'), mode: runtimeMode })}
            </span>
            <GatewayServiceControl onStateChange={handleGatewayStateChange} />
          </div>
          <div className="flex flex-wrap items-center justify-between gap-1" style={cardLabel}>
            <span className="tabular">{status ? `${t('dash.uptimePrefix')} ${fmtUptime(status.uptime_s)} · v${status.version}` : '—'}</span>
            <a href="#/settings" className="inline-flex items-center gap-1" style={{ color: 'var(--text-muted)' }} title={copy('instance.manage')} aria-label={copy('instance.manage')}><Settings size={14} aria-hidden="true" /></a>
          </div>
          <div className="mt-2 min-w-0 truncate text-xs" style={{ color: 'var(--text-muted)' }} title={apiAddress}>{t('dash.apiAddress')}: <code className="font-mono">{apiAddress}</code></div>
        </section>

        <a href="#/stats" className="fade-in card-hover block p-4 sm:p-5" style={{ ...card, ...layoutStyle('active-requests') }} title={t('dash.openActiveRequests')}>
          <div className="mb-2 flex items-center gap-2 font-semibold" style={{ fontSize: 'var(--text-lg)' }}>
            <SummaryMark icon={Activity} />{t('telemetry.active')}
          </div>
          <div className="tabular font-semibold" style={{ fontSize: 'var(--text-2xl)' }}>{activeRequests ?? '—'}</div>
          <div className="mt-2 flex items-center gap-1 text-xs" style={{ color: 'var(--text-muted)' }}>{t('dash.openActiveRequests')}<ArrowRight size={13} aria-hidden="true" /></div>
        </a>

        {/* provider 连通性探测（不是推理健康检查） */}
        <a href="#/providers" className="fade-in card-hover block p-4 sm:p-5" style={{ ...card, ...layoutStyle('providers') }}>
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
        <a href="#/routing" className="fade-in card-hover block p-4 sm:p-5" style={{ ...card, ...layoutStyle('routes') }}>
          <div className="mb-2 flex items-center gap-2 font-semibold" style={{ fontSize: 'var(--text-lg)' }}>
            <SummaryMark icon={GitBranch} />
            {routes === null ? '—' : t('dash.routes', { n: routes.length })}
          </div>
          <div style={cardLabel} className="tabular">
            {routes === null ? '—' : t('dash.routesSummary', { exact: exactCount, prefix: prefixCount })}
          </div>
          <div className="mt-2 inline-flex items-center gap-1" style={{ fontSize: 'var(--text-sm)', color: 'var(--text-muted)' }}>
            {t('dash.editRoutes')}<ArrowRight size={14} aria-hidden="true" />
          </div>
        </a>
      <div className="dashboard-layout-slot dashboard-layout-slot--telemetry" style={layoutStyle('telemetry')}><RuntimeTelemetry onActiveRequestsChange={setActiveRequests} /></div>

      {/* ③ 快捷操作 */}
      <div className="fade-in flex flex-wrap gap-3 dashboard-layout-slot dashboard-layout-slot--actions" style={layoutStyle('quick-actions')}>
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
    </div>
  );
}
