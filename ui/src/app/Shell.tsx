import { useEffect, useState, type ReactNode } from 'react';
import { BASE, fetchHealth } from '../api';
import { setAuthErrorHandler } from '../api/admin';
import { ConflictBanner } from '../components/ConflictBanner';
import { AdminLogin } from '../components/AdminLogin';
import { FeedbackProvider, useConflictControl, useToast } from './feedback';
import { useI18n } from '../i18n';
import { ThemeToggle } from '../components/ui/ThemeToggle';
import { LangToggle } from '../components/ui/LangToggle';
import pkg from '../../package.json';

/* ---------- hash 路由（手写，不引第三方 router） ---------- */

export type Route = 'home' | 'dashboard' | 'providers' | 'routing' | 'playground';

const ROUTES: readonly Route[] = ['home', 'dashboard', 'providers', 'routing', 'playground'];
const DEFAULT_ROUTE: Route = 'dashboard';

function parseHash(hash: string): Route {
  const path = hash.replace(/^#\/?/, '').split(/[?#]/)[0];
  return (ROUTES as readonly string[]).includes(path) ? (path as Route) : DEFAULT_ROUTE;
}

/** `/#/home`（默认）· `/#/providers` · `/#/routing` · `/#/playground` */
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

export function useServerStatus(): { status: ServerStatus; text: string } {
  const [status, setStatus] = useState<ServerStatus>('unknown');
  const [text, setText] = useState<string>('');

  useEffect(() => {
    let cancelled = false;
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
      }
    };
    probe();
    const timer = window.setInterval(probe, 8000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, []);

  return { status, text };
}

/* ---------- 壳 ---------- */

interface ShellProps {
  route: Route;
  children: ReactNode;
}

const NAV: ReadonlyArray<{ route: Route; labelKey: 'shell.navDashboard' | 'shell.navProviders' | 'shell.navRouting' | 'shell.navPlayground'; href: string }> = [
  { route: 'dashboard', labelKey: 'shell.navDashboard', href: '#/dashboard' },
  { route: 'providers', labelKey: 'shell.navProviders', href: '#/providers' },
  { route: 'routing', labelKey: 'shell.navRouting', href: '#/routing' },
  { route: 'playground', labelKey: 'shell.navPlayground', href: '#/playground' },
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
  const server = useServerStatus();
  const { conflict, handlers } = useConflictControl();
  const { toasts } = useToast();
  const { t } = useI18n();
  const endpointLabel = BASE.replace(/^https?:\/\//, '');

  // #43 cloud 态：任一 admin 请求 401/403 → 登录小窗；成功后整页刷新重拉
  // （极简：避免逐页接线重拉逻辑；local 态服务端不产 401，回调永不触发）。
  const [needLogin, setNeedLogin] = useState(false);
  useEffect(() => {
    setAuthErrorHandler(() => setNeedLogin(true));
    return () => setAuthErrorHandler(null);
  }, []);

  const daemonLabel =
    server.status === 'ok'
      ? t('shell.daemonOk')
      : server.status === 'error'
        ? t('shell.daemonUnreachable')
        : t('shell.connecting');

  return (
    <div className="flex min-h-full flex-col bg-bg text-ink">
      {needLogin && (
        <AdminLogin
          onSuccess={() => {
            setNeedLogin(false);
            window.location.reload();
          }}
        />
      )}
      {/* 顶栏（v2.0 单行）：logo + 导航 + 右侧状态/切换。
          旧版第二行的 masked-key / endpoint / file=truth 等技术债文案已删
          （审查报告 §1 冗余说明文字）。 */}
      <header
        className="border-b"
        style={{ borderColor: 'var(--border)', background: 'var(--surface)' }}
      >
        <div className="mx-auto flex max-w-7xl flex-wrap items-center justify-between gap-4 px-6 py-3">
          <div className="flex items-center gap-6">
            <a
              href="#/dashboard"
              className="flex items-center gap-2 font-semibold"
              style={{ fontSize: 'var(--text-base)', color: 'var(--text)' }}
            >
              <span
                className="inline-flex h-6 w-6 items-center justify-center font-bold text-white"
                style={{
                  background: 'var(--accent)',
                  borderRadius: 'var(--radius)',
                  fontSize: 'var(--text-sm)',
                }}
              >
                J
              </span>
              <span>Jev-Switch</span>
            </a>
            <nav aria-label="primary" className="flex items-center gap-1">
              {NAV.map((item) => {
                const active =
                  item.route === route || (item.route === 'dashboard' && route === 'home');
                return (
                  <a
                    key={item.route}
                    href={item.href}
                    aria-current={active ? 'page' : undefined}
                    className="px-3 py-1.5 transition-colors"
                    style={{
                      fontSize: 'var(--text-sm)',
                      borderRadius: 'var(--radius)',
                      fontWeight: active ? 600 : 400,
                      color: active ? 'var(--text)' : 'var(--text-muted)',
                      background: active ? 'var(--surface-hover)' : 'transparent',
                    }}
                  >
                    {t(item.labelKey)}
                  </a>
                );
              })}
            </nav>
          </div>
          <div className="flex items-center gap-3">
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
            <ThemeToggle />
            <LangToggle />
          </div>
        </div>
      </header>

      {/* 冲突横幅槽位（状态由页面 raise 接入） */}
      {conflict && handlers && (
        <ConflictBanner onReload={handlers.reload} onOverwrite={handlers.overwrite} />
      )}

      {/* 页面 */}
      <main className="flex-1">{children}</main>

      {/* footer：只留 endpoint 一项（可 hover 看全），去掉 file=truth 等内部术语 */}
      <footer className="border-t" style={{ borderColor: 'var(--border)' }}>
        <div
          className="mx-auto max-w-7xl px-6 py-3"
          style={{
            fontSize: 'var(--text-xs)',
            color: 'var(--text-subtle)',
            fontFamily: 'var(--font-mono)',
          }}
          title="Daemon endpoint · POST /v1/systemone"
        >
          {endpointLabel}
        </div>
      </footer>

      {/* toast 槽位 — B1 tint 胶囊 */}
      {toasts.length > 0 && (
        <div className="fixed bottom-4 right-4 z-50 flex w-80 flex-col gap-2" aria-live="polite">
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
  );
}
