import { useCallback, useEffect, useState } from 'react';
import { fetchModels } from '../api';
import {
  clearAdminSession,
  getStatus,
  listProviders,
  listRoutes,
  putListen,
  putMode,
  putPassword,
  type AdminProvider,
  type AdminStatus,
  type Route,
} from '../api/admin';
import { useServerStatus } from '../app/Shell';
import { useToast } from '../app/feedback';
import { StatusBadge, type BadgeTone } from '../components/ui/StatusBadge';
import { ThemeToggle } from '../components/ui/ThemeToggle';
import { LangToggle } from '../components/ui/LangToggle';
import { useI18n, type MessageKey } from '../i18n';

/**
 * 首页 Home（块 5 · docs/12 裁决 #9，IA 三页→四页，默认 #/home）。
 * ① 状态仪表盘：系统卡（daemon/version/uptime/mode/bind/env 警示/设密态）+
 *    提供商卡（N/M + service-row 列表）+ 路由卡（models/edges/prefix）+ Run Jev 入口。
 * ② 统一操作区：mode 切换（cloud 激活 400 → 设密对话框带密原子上锁）、
 *    listen 显示/编辑（auto 恢复成对默认，失败旧监听保持）、主题/语言切换
 *    （与 Shell 顶栏同组件两处渲染）、快捷磁贴。
 * 数据：GET /v1/admin/status（7 键，ts-rs 单源）+ listProviders/listRoutes/fetchModels；
 * status 404/mock → fixture（沿用 adminMode 开关，Mode 合入即切真）。
 */

type LoadState<T> = { data: T | null; error: string | null };

function fmtUptime(sec: number, t: (k: MessageKey, v?: Record<string, string | number>) => string): string {
  const d = Math.floor(sec / 86400);
  const h = Math.floor((sec % 86400) / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = sec % 60;
  if (d > 0) return t('home.uptimeD', { d, h, m });
  if (h > 0) return t('home.uptimeH', { h, m, s });
  if (m > 0) return t('home.uptimeM', { m, s });
  return t('home.uptimeS', { s });
}

export function HomePage() {
  const { t } = useI18n();
  const { toast } = useToast();
  const server = useServerStatus();

  const [status, setStatus] = useState<LoadState<AdminStatus & { source?: 'live' | 'mock' }>>({
    data: null,
    error: null,
  });
  const [providers, setProviders] = useState<LoadState<AdminProvider[]>>({ data: null, error: null });
  const [routes, setRoutes] = useState<LoadState<Route[]>>({ data: null, error: null });
  const [modelCount, setModelCount] = useState<number | null>(null);

  const [modeBusy, setModeBusy] = useState(false);
  const [pendingMode, setPendingMode] = useState<string | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [dialogPurpose, setDialogPurpose] = useState<'mode' | 'password'>('mode');
  const [dialogPw, setDialogPw] = useState('');
  const [dialogErr, setDialogErr] = useState<string | null>(null);
  const [dialogBusy, setDialogBusy] = useState(false);

  const [listenEditing, setListenEditing] = useState(false);
  const [listenAddr, setListenAddr] = useState('');
  const [listenBusy, setListenBusy] = useState(false);

  const loadStatus = useCallback(async () => {
    try {
      const r = await getStatus();
      setStatus({ data: r.source === 'mock' ? { ...r.status, source: 'mock' } : r.status, error: null });
      if (r.source === 'live') setListenAddr((prev) => prev || r.status.bind);
    } catch (e) {
      setStatus({ data: null, error: (e as Error).message });
    }
  }, []);

  const loadAll = useCallback(async () => {
    void loadStatus();
    try {
      const r = await listProviders();
      setProviders({ data: r.providers, error: null });
    } catch (e) {
      setProviders({ data: null, error: (e as Error).message });
    }
    try {
      const r = await listRoutes();
      setRoutes({ data: r.routes, error: null });
    } catch (e) {
      setRoutes({ data: null, error: (e as Error).message });
    }
    try {
      const m = await fetchModels();
      setModelCount(Array.isArray(m.data) ? m.data.length : 0);
    } catch {
      setModelCount(null);
    }
  }, [loadStatus]);

  useEffect(() => {
    void loadAll();
  }, [loadAll]);

  /* ---- mode 切换：确认 → PUT；400 admin password required → 设密对话框带密重发 ---- */
  const runModeSwitch = async (mode: string, adminPassword: string | null) => {
    setModeBusy(true);
    setDialogBusy(true);
    try {
      const res = await putMode({ mode, admin_password: adminPassword });
      const rb = res.rebind;
      toast(
        'ok',
        'skipped' in rb
          ? t('home.rebindSkipped', { mode: res.mode, reason: rb.skipped })
          : t('home.rebindOk', { mode: res.mode, from: rb.from, to: rb.to }),
      );
      setPendingMode(null);
      setDialogOpen(false);
      setDialogPw('');
      setDialogErr(null);
      await loadStatus();
    } catch (e) {
      const msg = (e as Error).message;
      if (adminPassword === null && msg.includes('admin password required')) {
        // cloud 激活密码闸 → 弹设密对话框（提交时带密重发，一发原子上锁）
        setDialogPurpose('mode');
        setDialogOpen(true);
        setDialogErr(null);
      } else if (dialogOpen && dialogPurpose === 'mode' && adminPassword !== null) {
        setDialogErr(msg);
      } else {
        toast('danger', t('home.modeFailed', { msg }));
        setPendingMode(null);
      }
    } finally {
      setModeBusy(false);
      setDialogBusy(false);
    }
  };

  const submitDialog = async (e: React.FormEvent) => {
    e.preventDefault();
    if (dialogPw.length === 0 || dialogBusy) return;
    if (dialogPurpose === 'mode') {
      await runModeSwitch(pendingMode ?? 'cloud', dialogPw);
      return;
    }
    // 首密引导：PUT /v1/admin/password → 全员重登录 → 清本地会话
    setDialogBusy(true);
    try {
      await putPassword(dialogPw);
      clearAdminSession();
      toast('ok', t('home.pwSet'));
      setDialogOpen(false);
      setDialogPw('');
      setDialogErr(null);
      await loadStatus();
    } catch (err) {
      setDialogErr(t('home.pwFailed', { msg: (err as Error).message }));
    } finally {
      setDialogBusy(false);
    }
  };

  const openPasswordGuide = () => {
    setDialogPurpose('password');
    setDialogPw('');
    setDialogErr(null);
    setDialogOpen(true);
  };

  /* ---- listen 编辑 ---- */
  const saveListen = async (addr: string) => {
    setListenBusy(true);
    try {
      const res = await putListen(addr);
      toast('ok', t('home.listenSaved', { addr: res.addr }));
      setListenEditing(false);
      await loadStatus();
    } catch (e) {
      toast('danger', t('home.listenFailed', { msg: (e as Error).message }));
    } finally {
      setListenBusy(false);
    }
  };

  const st = status.data;
  const modeTone: BadgeTone = 'info';
  const daemonTone: BadgeTone =
    server.status === 'ok' ? 'ok' : server.status === 'error' ? 'danger' : 'muted';
  const daemonLabel =
    server.status === 'ok'
      ? t('shell.daemonOk')
      : server.status === 'error'
        ? t('shell.daemonUnreachable')
        : t('shell.connecting');

  const enabledCount = providers.data?.filter((p) => p.enabled).length ?? 0;
  const totalCount = providers.data?.length ?? 0;
  const prefixCount = routes.data?.filter((r) => r.match === 'prefix').length ?? 0;
  const visibleProviders = (providers.data ?? []).slice(0, 3);
  const moreProviders = Math.max((providers.data?.length ?? 0) - visibleProviders.length, 0);

  const panelHead = (title: string, cap?: React.ReactNode) => (
    <div className="flex items-center justify-between gap-2 border-b border-border px-4 py-2.5">
      <span className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkMuted">
        {title}
      </span>
      {cap}
    </div>
  );

  const quickTile = (label: string, href: string, primary = false) => (
    <a
      href={href}
      className={
        'flex h-10 items-center justify-center rounded-ctl border px-3 font-mono text-xs transition-colors ' +
        (primary
          ? 'border-primaryFill bg-primaryFill text-white hover:bg-primaryFillHover hover:border-primaryFillHover'
          : 'border-border bg-panel text-ink hover:border-primaryBright hover:bg-soft')
      }
    >
      {label}
    </a>
  );

  return (
    <div className="mx-auto max-w-7xl space-y-4 px-6 py-8">
      {/* 页头 */}
      <div className="flex items-baseline justify-between">
        <h1 className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkMuted">
          {t('shell.navHome')}
        </h1>
        {st?.source === 'mock' && (
          <span className="font-mono text-[10px] uppercase tracking-widest text-inkMuted">
            {t('home.mockStatus')}
          </span>
        )}
      </div>

      {/* ① 状态仪表盘 */}
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {/* 系统卡 */}
        <section className="overflow-hidden rounded-card border border-border bg-panel xl:col-span-2">
          {panelHead(
            t('home.systemCard'),
            <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
              GET /v1/admin/status
            </span>,
          )}
          <div className="divide-y divide-border">
            <div className="grid grid-cols-[110px_1fr] items-center gap-3 px-4 py-2.5">
              <span className="font-mono text-[10px] uppercase tracking-widest text-inkMuted">daemon</span>
              <StatusBadge tone={daemonTone} aria-live="polite">
                {daemonLabel}
              </StatusBadge>
            </div>
            <div className="grid grid-cols-[110px_1fr] items-center gap-3 px-4 py-2.5 font-mono text-xs tabular">
              <span className="text-[10px] uppercase tracking-widest text-inkMuted">
                {t('home.version')}
              </span>
              <span className="text-ink">{st ? `v${st.version}` : '—'}</span>
            </div>
            <div className="grid grid-cols-[110px_1fr] items-center gap-3 px-4 py-2.5 font-mono text-xs tabular">
              <span className="text-[10px] uppercase tracking-widest text-inkMuted">
                {t('home.uptime')}
              </span>
              <span className="text-ink">{st ? fmtUptime(st.uptime_s, t) : '—'}</span>
            </div>
            <div className="grid grid-cols-[110px_1fr] items-center gap-3 px-4 py-2.5">
              <span className="font-mono text-[10px] uppercase tracking-widest text-inkMuted">
                {t('home.mode')}
              </span>
              <span className="flex flex-wrap items-center gap-2">
                <span
                  className={
                    'inline-flex items-center gap-1.5 whitespace-nowrap rounded-full px-2 py-0.5 font-mono text-[11px] font-semibold ' +
                    (st?.mode === 'cloud'
                      ? 'bg-primaryFill text-white'
                      : 'bg-infoBg text-info')
                  }
                >
                  {st?.mode ?? '—'}
                </span>
                {st && !st.password_set && (
                  <StatusBadge tone={st.mode === 'cloud' ? 'danger' : 'warn'}>
                    {t('home.pwUnset')}
                  </StatusBadge>
                )}
                {st?.password_set && <StatusBadge tone="ok">{t('home.pwSet')}</StatusBadge>}
                {st?.env_override_active && (
                  <StatusBadge tone="warn">{t('home.envOverride')}</StatusBadge>
                )}
                {!st?.password_set && (
                  <button
                    type="button"
                    onClick={openPasswordGuide}
                    className="h-7 rounded-ctl border border-border bg-panel px-2 font-mono text-[10px] uppercase tracking-widest text-ink transition-colors hover:border-primaryBright hover:bg-soft"
                  >
                    {t('home.setPassword')}
                  </button>
                )}
              </span>
            </div>
            <div className="grid grid-cols-[110px_1fr] items-center gap-3 px-4 py-2.5 font-mono text-xs">
              <span className="text-[10px] uppercase tracking-widest text-inkMuted">
                {t('home.bind')}
              </span>
              <span className="truncate text-ink tabular">{st?.bind ?? '—'}</span>
            </div>
          </div>
          {status.error && (
            <div className="border-t border-danger bg-dangerBg px-4 py-2 font-mono text-xs text-danger">
              {t('common.loadFailed')}
              {status.error}
            </div>
          )}
        </section>

        {/* 提供商卡 */}
        <a
          href="#/providers"
          className="group overflow-hidden rounded-card border border-border bg-panel transition-colors hover:border-primaryBright"
        >
          {panelHead(
            t('shell.navProviders'),
            <span className="font-mono text-[10px] uppercase tracking-widest text-inkMuted tabular">
              {providers.data
                ? t('prov.enabledCount', { n: enabledCount, total: totalCount })
                : '—'}
            </span>,
          )}
          <div className="divide-y divide-border">
            {providers.error ? (
              <div className="px-4 py-3 font-mono text-xs text-danger">
                {t('common.loadFailed')}
                {providers.error}
              </div>
            ) : providers.data === null ? (
              <div className="h-24 animate-pulse bg-soft" aria-busy="true" />
            ) : visibleProviders.length === 0 ? (
              <div className="px-4 py-4 text-xs text-inkMuted">{t('prov.empty')}</div>
            ) : (
              visibleProviders.map((p) => (
                <div
                  key={p.id}
                  className="flex items-center gap-2.5 px-4 py-2.5 font-mono text-xs transition-colors group-hover:bg-soft"
                >
                  <span
                    className={
                      'inline-block h-1.5 w-1.5 shrink-0 rounded-full ' +
                      (p.enabled ? 'bg-okDot' : 'bg-border')
                    }
                    aria-hidden
                  />
                  <span className="w-16 shrink-0 truncate font-semibold text-ink tabular">{p.id}</span>
                  <span className="min-w-0 flex-1 truncate text-inkMuted" title={p.base}>
                    {p.base}
                  </span>
                </div>
              ))
            )}
            {moreProviders > 0 && (
              <div className="px-4 py-2 font-mono text-[10px] uppercase tracking-widest text-inkMuted">
                {t('home.moreRows', { n: moreProviders })}
              </div>
            )}
          </div>
        </a>

        {/* 路由卡 */}
        <a
          href="#/routing"
          className="group overflow-hidden rounded-card border border-border bg-panel transition-colors hover:border-primaryBright"
        >
          {panelHead(
            t('shell.navRouting'),
            <span className="font-mono text-[10px] uppercase tracking-widest text-inkMuted tabular">
              {routes.data ? `${routes.data.length} edges` : '—'}
            </span>,
          )}
          <div className="px-4 py-4">
            <p className="font-mono text-sm text-ink tabular">
              {routes.data === null ? (
                <span className="text-inkMuted">—</span>
              ) : (
                t('home.routesSummary', {
                  models: modelCount ?? '—',
                  edges: routes.data.length,
                  prefix: prefixCount,
                })
              )}
            </p>
            <p className="mt-2 font-mono text-[10px] uppercase tracking-widest text-inkMuted transition-colors group-hover:text-primary">
              {t('shell.navRouting')} →
            </p>
          </div>
        </a>
      </div>

      {/* Run Jev 请求入口 */}
      <section className="overflow-hidden rounded-card border border-border bg-panel">
        {panelHead(
          t('home.runCard'),
          <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
            POST /v1/systemone
          </span>,
        )}
        <div className="flex flex-wrap items-center justify-between gap-3 px-4 py-4">
          <p className="text-sm text-inkMuted">One protocol. Every upstream.</p>
          <a
            href="#/playground"
            className="inline-flex h-10 items-center gap-2 rounded-ctl border border-primaryFill bg-primaryFill px-6 font-mono text-sm font-semibold text-white transition-colors hover:bg-primaryFillHover hover:border-primaryFillHover"
          >
            {t('home.tileRun')}
          </a>
        </div>
      </section>

      {/* ② 统一操作区 */}
      <div className="grid gap-4 lg:grid-cols-2">
        {/* 模式切换 */}
        <section className="overflow-hidden rounded-card border border-border bg-panel">
          {panelHead(
            t('home.modeSection'),
            st ? (
              <span className="font-mono text-[10px] uppercase tracking-widest text-primary">
                {st.mode}
              </span>
            ) : undefined,
          )}
          <div className="space-y-3 px-4 py-4">
            <div className="flex flex-wrap items-center gap-2">
              {(['local', 'cloud'] as const).map((m) => {
                const active = st?.mode === m;
                const armed = pendingMode === m;
                return (
                  <button
                    key={m}
                    type="button"
                    aria-pressed={active}
                    disabled={modeBusy || st === null}
                    onClick={() => {
                      if (active) {
                        setPendingMode(null);
                        return;
                      }
                      setPendingMode(m);
                      setDialogErr(null);
                    }}
                    className={
                      'h-9 min-w-24 rounded-ctl border px-4 font-mono text-xs font-semibold uppercase tracking-widest transition-colors disabled:opacity-50 ' +
                      (active
                        ? 'border-primaryFill bg-primaryFill text-white'
                        : armed
                          ? 'border-primaryBright bg-infoBg text-info'
                          : 'border-border bg-panel text-ink hover:border-primaryBright hover:bg-soft')
                    }
                  >
                    {m}
                  </button>
                );
              })}
              <span className="font-mono text-[10px] uppercase tracking-widest text-inkMuted">
                {modeTone ? t('home.modeSection') : ''}
              </span>
            </div>

            {pendingMode && (
              <div className="flex flex-wrap items-center gap-2 rounded-ctl border border-primaryBright bg-infoBg px-3 py-2">
                <span className="font-mono text-xs text-info">
                  {t('home.switchTo', { mode: pendingMode })}
                </span>
                <button
                  type="button"
                  disabled={modeBusy}
                  onClick={() => void runModeSwitch(pendingMode, null)}
                  className="h-7 rounded-ctl border border-primaryFill bg-primaryFill px-3 font-mono text-[11px] font-semibold text-white hover:bg-primaryFillHover hover:border-primaryFillHover disabled:opacity-50"
                >
                  {t('home.confirm')}
                </button>
                <button
                  type="button"
                  disabled={modeBusy}
                  onClick={() => setPendingMode(null)}
                  className="h-7 rounded-ctl border border-border bg-panel px-3 font-mono text-[11px] text-inkMuted hover:border-primaryBright hover:text-ink disabled:opacity-50"
                >
                  {t('common.cancel')}
                </button>
              </div>
            )}

            <p className="font-mono text-[10px] leading-relaxed uppercase tracking-widest text-inkMuted">
              {t('home.modeHint')}
            </p>
          </div>
        </section>

        {/* listen 显示/编辑 */}
        <section className="overflow-hidden rounded-card border border-border bg-panel">
          {panelHead(
            t('home.listenSection'),
            <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
              PUT /v1/admin/listen
            </span>,
          )}
          <div className="space-y-3 px-4 py-4">
            {listenEditing ? (
              <div className="flex flex-wrap items-center gap-2">
                <input
                  value={listenAddr}
                  onChange={(e) => setListenAddr(e.target.value)}
                  placeholder={st?.bind ?? '127.0.0.1:11435'}
                  spellCheck={false}
                  autoFocus
                  aria-label={t('home.listenSection')}
                  className="h-9 flex-1 min-w-48 rounded-ctl border border-border bg-soft px-3 font-mono text-xs text-ink tabular placeholder:text-inkSubtle"
                />
                <button
                  type="button"
                  disabled={listenBusy || listenAddr.trim().length === 0}
                  onClick={() => void saveListen(listenAddr.trim())}
                  className="h-9 rounded-ctl border border-primaryFill bg-primaryFill px-3 font-mono text-xs font-semibold text-white hover:bg-primaryFillHover hover:border-primaryFillHover disabled:opacity-50"
                >
                  {t('common.save')}
                </button>
                <button
                  type="button"
                  disabled={listenBusy}
                  onClick={() => setListenEditing(false)}
                  className="h-9 rounded-ctl border border-border bg-panel px-3 font-mono text-xs text-inkMuted hover:border-primaryBright hover:text-ink disabled:opacity-50"
                >
                  {t('common.cancel')}
                </button>
                <button
                  type="button"
                  disabled={listenBusy}
                  onClick={() => void saveListen('auto')}
                  className="h-9 rounded-ctl border border-border bg-panel px-3 font-mono text-xs text-ink hover:border-primaryBright hover:bg-soft disabled:opacity-50"
                >
                  {t('home.listenAuto')}
                </button>
              </div>
            ) : (
              <div className="flex flex-wrap items-center justify-between gap-2">
                <code className="rounded border border-border bg-soft px-2 py-1 font-mono text-xs text-ink tabular">
                  {st?.bind ?? '—'}
                </code>
                <button
                  type="button"
                  disabled={st === null}
                  onClick={() => {
                    setListenAddr(st?.bind ?? '');
                    setListenEditing(true);
                  }}
                  className="h-8 rounded-ctl border border-border bg-panel px-3 font-mono text-xs text-ink hover:border-primaryBright hover:bg-soft disabled:opacity-50"
                >
                  {t('home.listenEdit')}
                </button>
              </div>
            )}
            <p className="font-mono text-[10px] leading-relaxed uppercase tracking-widest text-inkMuted">
              {t('home.listenHint')}
            </p>
          </div>
        </section>

        {/* 外观与语言 — 与 Shell 顶栏同组件 */}
        <section className="overflow-hidden rounded-card border border-border bg-panel">
          {panelHead(t('home.appearanceSection'))}
          <div className="flex flex-wrap items-center gap-3 px-4 py-4">
            <ThemeToggle />
            <LangToggle />
            <span className="font-mono text-[10px] uppercase tracking-widest text-inkMuted">
              {t('home.appearanceHint')}
            </span>
          </div>
        </section>

        {/* 快捷磁贴 */}
        <section className="overflow-hidden rounded-card border border-border bg-panel">
          {panelHead(t('home.quickSection'))}
          <div className="grid gap-2 px-4 py-4 sm:grid-cols-3">
            {quickTile(t('prov.add'), '#/providers')}
            {quickTile(t('shell.navRouting'), '#/routing')}
            {quickTile(t('home.tileRun'), '#/playground', true)}
          </div>
        </section>
      </div>

      {/* 设密对话框（mode 激活 / 首密引导共用；仅 password 输入，无回显） */}
      {dialogOpen && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-overlay"
          role="dialog"
          aria-modal="true"
          aria-labelledby="home-pw-title"
        >
          <form
            onSubmit={(e) => void submitDialog(e)}
            className="w-full max-w-sm rounded-card border border-border bg-panel p-6"
          >
            <h2
              id="home-pw-title"
              className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkMuted"
            >
              {t('home.setPassword')}
            </h2>
            <p className="mt-2 font-mono text-xs leading-relaxed text-inkMuted">
              {t('home.setPasswordHint')}
            </p>
            <input
              type="password"
              value={dialogPw}
              onChange={(e) => setDialogPw(e.target.value)}
              placeholder={t('home.pwLabel')}
              autoComplete="new-password"
              spellCheck={false}
              autoFocus
              className="mt-4 h-9 w-full rounded-ctl border border-border bg-soft px-3 font-mono text-xs text-ink placeholder:text-inkSubtle focus:border-primaryBright focus:outline-none"
            />
            {dialogErr && (
              <p className="mt-2 font-mono text-xs text-danger" role="alert">
                {dialogErr}
              </p>
            )}
            <div className="mt-4 flex justify-end gap-2">
              <button
                type="button"
                onClick={() => {
                  setDialogOpen(false);
                  setDialogPw('');
                  setDialogErr(null);
                  if (dialogPurpose === 'mode') setPendingMode(null);
                }}
                className="h-8 rounded-ctl border border-border bg-panel px-4 font-mono text-xs text-inkMuted hover:border-primaryBright hover:text-ink"
              >
                {t('common.cancel')}
              </button>
              <button
                type="submit"
                disabled={dialogBusy || dialogPw.length === 0}
                className="h-8 rounded-ctl border border-primaryFill bg-primaryFill px-4 font-mono text-xs font-semibold text-white hover:bg-primaryFillHover hover:border-primaryFillHover disabled:cursor-not-allowed disabled:opacity-50"
              >
                {dialogPurpose === 'mode' ? t('home.activate') : t('common.save')}
              </button>
            </div>
          </form>
        </div>
      )}
    </div>
  );
}
