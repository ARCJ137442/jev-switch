import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { BarChart3, Boxes, DoorOpen, FlaskConical, LayoutDashboard, Route, Settings, type LucideIcon } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { isTauri } from '@tauri-apps/api/core';
import { getBase, fetchHealth } from '../api';
import { clearAdminSession, getAdminSession, setAuthErrorHandler } from '../api/admin';
import { ConflictBanner } from '../components/ConflictBanner';
import { AdminLogin } from '../components/AdminLogin';
import { FeedbackProvider, useConflictControl, useToast } from './feedback';
import { useI18n } from '../i18n';
import { CommandPalette } from '../components/commands/CommandPalette';
import pkg from '../../package.json';
import { useAuth } from '../auth/AuthContext';
import { readShowStatusBar, SETTINGS_CHANGE_EVENT } from '../settings/preferences';
import { StatusBarContributionProvider, type StatusBarItem } from './statusBar';
import './shell.css';

/* ---------- hash 路由（手写，不引第三方 router） ---------- */

export type Route = 'home' | 'dashboard' | 'providers' | 'endpoints' | 'routing' | 'playground' | 'stats' | 'settings';

const ROUTES: readonly Route[] = ['home', 'dashboard', 'providers', 'endpoints', 'routing', 'playground', 'stats', 'settings'];
const DEFAULT_ROUTE: Route = 'dashboard';

function parseHash(hash: string): Route {
  const path = hash.replace(/^#\/?/, '').split(/[?#]/)[0];
  return (ROUTES as readonly string[]).includes(path) ? (path as Route) : DEFAULT_ROUTE;
}

/** `/#/dashboard`（默认）· `/#/providers` · `/#/endpoints` · `/#/routing` · `/#/playground` */
export function useHashRoute(): Route {
  const [route, setRoute] = useState<Route>(() =>
    typeof window === 'undefined' ? DEFAULT_ROUTE : parseHash(window.location.hash),
  );
  useEffect(() => {
    const onChange = () => setRoute(parseHash(window.location.hash));
    window.addEventListener('hashchange', onChange);
    return () => window.removeEventListener('hashchange', onChange);
  }, []);
  return route;
}

/* ---------- daemon 状态（health 轮询，页面可复用） ---------- */

export type ServerStatus = 'unknown' | 'ok' | 'error';

export interface ServerStatusSnapshot {
  status: ServerStatus;
  text: string;
}

const ServerStatusContext = createContext<ServerStatusSnapshot>({ status: 'unknown', text: '' });

export function useServerStatus(): ServerStatusSnapshot {
  return useContext(ServerStatusContext);
}

function useServerStatusProbe(): ServerStatusSnapshot {
  const [status, setStatus] = useState<ServerStatus>('unknown');
  const [text, setText] = useState<string>('');

  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;
    const probe = async () => {
      try {
        const health = await fetchHealth();
        if (!cancelled) {
          setStatus('ok');
          setText(health.version ? `v${health.version}` : 'ok');
        }
      } catch (e) {
        if (!cancelled) {
          setStatus('error');
          setText((e as Error).message);
        }
      } finally {
        if (!cancelled) timer = window.setTimeout(() => void probe(), 8000);
      }
    };
    void probe();
    return () => {
      cancelled = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, []);

  return { status, text };
}

/* ---------- 壳 ---------- */

interface ShellProps {
  route: Route;
  children: ReactNode;
}

const NAV: ReadonlyArray<{ route: Route; labelKey: 'shell.navDashboard' | 'shell.navProviders' | 'entry.tab' | 'shell.navRouting' | 'shell.navPlayground' | 'shell.navStats' | 'shell.navSettings'; href: string; icon: LucideIcon }> = [
  { route: 'dashboard', labelKey: 'shell.navDashboard', href: '#/dashboard', icon: LayoutDashboard },
  { route: 'providers', labelKey: 'shell.navProviders', href: '#/providers', icon: Boxes },
  { route: 'endpoints', labelKey: 'entry.tab', href: '#/endpoints', icon: DoorOpen },
  { route: 'routing', labelKey: 'shell.navRouting', href: '#/routing', icon: Route },
  { route: 'playground', labelKey: 'shell.navPlayground', href: '#/playground', icon: FlaskConical },
  { route: 'stats', labelKey: 'shell.navStats', href: '#/stats', icon: BarChart3 },
  { route: 'settings', labelKey: 'shell.navSettings', href: '#/settings', icon: Settings },
];

/** design/01 §5 版式 × TeamSense 导航激活态（A6）× 方案 B 色 */
export function Shell({ route, children }: ShellProps) {
  return (
    <FeedbackProvider>
      <ShellFrame route={route}>{children}</ShellFrame>
    </FeedbackProvider>
  );
}

function ShellFrame({ route, children }: ShellProps) {
  const server = useServerStatusProbe();
  const { conflict, handlers } = useConflictControl();
  const { toasts } = useToast();
  const { t } = useI18n();
  const auth = useAuth();
  const endpointLabel = (getBase() || window.location.origin).replace(/^https?:\/\//, '');

  // #43 cloud 态：任一 admin 请求 401/403 → 登录小窗；成功后整页刷新重拉
  // （极简：避免逐页接线重拉逻辑；local 态服务端不产 401，回调永不触发）。
  const [needLogin, setNeedLogin] = useState(false);
  const [contentVersion, setContentVersion] = useState(0);
  const [, setApiBaseVersion] = useState(0);
  const [showStatusBar, setShowStatusBar] = useState(readShowStatusBar);
  const [pageStatusItems, setPageStatusItems] = useState<readonly StatusBarItem[]>([]);
  const registerPageStatusItems = useCallback((items: readonly StatusBarItem[]) => setPageStatusItems(items), []);
  const currentNav = useMemo(() => NAV.find((item) => item.route === route) ?? NAV[0], [route]);
  useEffect(() => {
    setAuthErrorHandler(() => setNeedLogin(true));
    const onApiBaseChange = () => setApiBaseVersion((version) => version + 1);
    window.addEventListener('jev-api-base-change', onApiBaseChange);
    return () => {
      setAuthErrorHandler(null);
      window.removeEventListener('jev-api-base-change', onApiBaseChange);
    };
  }, []);

  useEffect(() => {
    const sync = () => setShowStatusBar(readShowStatusBar());
    window.addEventListener(SETTINGS_CHANGE_EVENT, sync);
    return () => window.removeEventListener(SETTINGS_CHANGE_EVENT, sync);
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'F11' || !isTauri()) return;
      if ((event.target as HTMLElement | null)?.closest('input,textarea,select,[contenteditable="true"]')) return;
      event.preventDefault();
      void getCurrentWindow().isFullscreen().then((fullscreen) => getCurrentWindow().setFullscreen(!fullscreen));
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  const daemonLabel =
    server.status === 'ok'
      ? t('shell.daemonOk')
      : server.status === 'error'
        ? t('shell.daemonUnreachable')
        : t('shell.connecting');

  return (
    <ServerStatusContext.Provider value={server}>
    <div className="app-shell flex h-full min-h-0 flex-col bg-bg text-ink">
      <CommandPalette />
      {needLogin && (
        <AdminLogin
          onCancel={() => {
            setNeedLogin(false);
            clearAdminSession();
            if (auth.identity?.kind === 'admin-session') auth.logoutCaller();
          }}
          onSuccess={() => {
            setNeedLogin(false);
            auth.markAdmin();
            setContentVersion((version) => version + 1);
          }}
        />
      )}
      {/* 顶栏（v2.0 单行）：logo + 导航 + 右侧状态/切换。
          旧版第二行的 masked-key / endpoint / file=truth 等技术债文案已删
          （审查报告 §1 冗余说明文字）。 */}
      <header
        className="app-shell__header shrink-0 border-b"
        style={{ borderColor: 'var(--border)', background: 'var(--surface)' }}
      >
        <div className="app-shell__header-inner">
            <a
              href="#/dashboard"
              className="app-shell__brand flex items-center gap-2 font-semibold"
              style={{ fontSize: 'var(--text-base)', color: 'var(--text)' }}
            >
              <span>Jev-Switch</span>
            </a>
            <nav aria-label="primary" className="app-shell__nav flex min-w-0 items-center gap-1">
              {NAV.filter((item) => !auth.isReadOnly || item.route === 'dashboard' || item.route === 'playground' || item.route === 'stats' || item.route === 'settings').map((item) => {
                const active =
                  item.route === route || (item.route === 'dashboard' && route === 'home');
                return (
                  <a
                    key={item.route}
                    href={item.href}
                    aria-current={active ? 'page' : undefined}
                    className="inline-flex shrink-0 items-center gap-1.5 px-3 py-1.5 transition-colors"
                    style={{
                      fontSize: 'var(--text-sm)',
                      borderRadius: 'var(--radius)',
                      fontWeight: active ? 600 : 400,
                      color: active ? 'var(--text)' : 'var(--text-muted)',
                      background: active ? 'var(--surface-hover)' : 'transparent',
                    }}
                  >
                    <item.icon size={15} strokeWidth={1.8} aria-hidden="true" />
                    {t(item.labelKey)}
                  </a>
                );
              })}
            </nav>
          <div className="app-shell__identity">
            {auth.identity?.kind === 'caller' && (
              <span className="flex min-w-0 items-center gap-2" style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>
                <span className="app-shell__identity-label min-w-0 font-mono" title={`${auth.identity.id} · ${t(auth.identity.role === 'readonly' ? 'access.roleReadonly' : 'access.roleAdmin')}`}>
                  <span className="app-shell__identity-id">{auth.identity.id}</span>
                  <span className="app-shell__identity-role"> · {t(auth.identity.role === 'readonly' ? 'access.roleReadonly' : 'access.roleAdmin')}</span>
                </span>
                <button className="app-shell__identity-action" type="button" onClick={auth.logoutCaller} style={{ color: 'var(--accent)' }}>
                  {t('access.clearToken')}
                </button>
              </span>
            )}
            {!auth.identity?.kind && !getAdminSession() && (
              <button className="app-shell__login-button" type="button" onClick={() => setNeedLogin(true)} style={{ color: 'var(--accent)', fontSize: 'var(--text-xs)' }}>
                {t('access.adminLogin')}
              </button>
            )}
          </div>
          <div className="app-shell__actions flex items-center gap-3">
            <span
              className="inline-flex items-center gap-2"
              style={{ fontSize: 'var(--text-sm)', color: 'var(--text-muted)' }}
              aria-live="polite"
              title={daemonLabel}
            >
              <span
                className={server.status === 'ok' ? '' : 'status-danger'}
                style={{
                  width: 8,
                  height: 8,
                  borderRadius: '50%',
                  background:
                    server.status === 'ok'
                      ? 'var(--success)'
                      : server.status === 'error'
                        ? 'var(--danger)'
                        : 'var(--text-subtle)',
                  display: 'inline-block',
                }}
                aria-hidden
              />
              <span className="tabular">v{pkg.version}</span>
            </span>
          </div>
        </div>
      </header>

      {/* 冲突横幅槽位（状态由页面 raise 接入） */}
      {conflict && handlers && (
        <ConflictBanner onReload={handlers.reload} onOverwrite={handlers.overwrite} />
      )}

      {/* 页面 */}
      <StatusBarContributionProvider register={registerPageStatusItems}>
        <main key={`${route}:${contentVersion}`} className={`app-shell__main min-h-0 flex-1${route === 'routing' ? ' app-shell__main--routing' : ''}`}>{children}</main>
      </StatusBarContributionProvider>

      {showStatusBar && <footer className="app-shell__footer shrink-0 border-t" style={{ borderColor: 'var(--border)' }}>
        <div className="app-shell__footer-inner" style={{ fontSize: 'var(--text-xs)', color: 'var(--text-subtle)', fontFamily: 'var(--font-mono)' }}>
          <span className="app-shell__footer-endpoint" title="Backend endpoint · POST /v1/systemone">{endpointLabel}</span>
          <span className="app-shell__footer-page"><currentNav.icon size={13} aria-hidden="true" />{t(currentNav.labelKey)}</span>
          <span className="app-shell__footer-contributions">
            {pageStatusItems.map((item) => {
              const className = `app-shell__status-item app-shell__status-item--${item.tone ?? 'default'}`;
              const content = <><span>{item.label}</span></>;
              return item.href
                ? <a key={item.id} className={className} href={item.href} title={item.title}>{content}</a>
                : <span key={item.id} className={className} title={item.title}>{content}</span>;
            })}
          </span>
          <span className={`app-shell__footer-daemon app-shell__footer-daemon--${server.status}`} title={daemonLabel} aria-label={daemonLabel}>
            <span aria-hidden="true" />{daemonLabel}
          </span>
        </div>
      </footer>}

      {/* toast 槽位 — B1 tint 胶囊 */}
      {toasts.length > 0 && (
        <div className="app-shell__toasts fixed bottom-4 right-4 z-50 flex w-80 flex-col gap-2" aria-live="polite">
          {toasts.map((item) => (
            <div
              key={item.id}
              className={
                'rounded-ctl border px-3 py-2 font-mono text-xs ' +
                (item.kind === 'ok'
                  ? 'border-transparent bg-okBg text-ok'
                  : item.kind === 'warn'
                    ? 'border-transparent bg-warnBg text-warn'
                    : 'border-transparent bg-dangerBg text-danger')
              }
            >
              {item.message}
            </div>
          ))}
        </div>
      )}
    </div>
    </ServerStatusContext.Provider>
  );
}
