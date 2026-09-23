import { useEffect, useState, type ReactNode } from 'react';
import { BASE, fetchHealth } from '../api';
import { setAuthErrorHandler } from '../api/admin';
import { ConflictBanner } from '../components/ConflictBanner';
import { AdminLogin } from '../components/AdminLogin';
import { StatusBadge, type BadgeTone } from '../components/ui/StatusBadge';
import { FeedbackProvider, useConflictControl, useToast } from './feedback';
import pkg from '../../package.json';

/* ---------- hash 路由（手写，不引第三方 router） ---------- */

export type Route = 'providers' | 'routing' | 'playground';

const ROUTES: readonly Route[] = ['providers', 'routing', 'playground'];
const DEFAULT_ROUTE: Route = 'playground';

function parseHash(hash: string): Route {
  const path = hash.replace(/^#\/?/, '').split(/[?#]/)[0];
  return (ROUTES as readonly string[]).includes(path) ? (path as Route) : DEFAULT_ROUTE;
}

/** `/#/providers` · `/#/routing` · `/#/playground`（默认） */
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

const NAV: ReadonlyArray<{ route: Route; label: string; href: string }> = [
  { route: 'providers', label: 'Providers', href: '#/providers' },
  { route: 'routing', label: 'Routing', href: '#/routing' },
  { route: 'playground', label: 'Playground', href: '#/playground' },
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
  const endpointLabel = BASE.replace(/^https?:\/\//, '');

  // #43 cloud 态：任一 admin 请求 401/403 → 登录小窗；成功后整页刷新重拉
  // （极简：避免逐页接线重拉逻辑；local 态服务端不产 401，回调永不触发）。
  const [needLogin, setNeedLogin] = useState(false);
  useEffect(() => {
    setAuthErrorHandler(() => setNeedLogin(true));
    return () => setAuthErrorHandler(null);
  }, []);

  /* 主题切换（块 2）：data-theme 由 main.tsx 首帧初始化；此处只读当前值 + 翻转持久化 */
  const [theme, setThemeState] = useState<'light' | 'dark'>(
    () =>
      (document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light') as
        | 'light'
        | 'dark',
  );
  const toggleTheme = () => {
    const next = theme === 'dark' ? 'light' : 'dark';
    document.documentElement.dataset.theme = next;
    try {
      localStorage.setItem('jev_theme', next);
    } catch {
      /* 隐私模式 → 仅本会话生效 */
    }
    setThemeState(next);
  };

  const daemonTone: BadgeTone =
    server.status === 'ok' ? 'ok' : server.status === 'error' ? 'danger' : 'muted';
  const daemonLabel =
    server.status === 'ok'
      ? 'daemon ok'
      : server.status === 'error'
        ? 'daemon unreachable'
        : 'connecting…';

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
      {/* 顶栏第一行：logo + 三页导航（激活 = panel 底 + 边 + 微影，TeamSense aria-current 手法） */}
      <header className="border-b border-border bg-bg">
        <div className="mx-auto flex max-w-7xl items-center justify-between gap-6 px-6 py-3">
          <a
            href="#/playground"
            className="flex items-center gap-2 font-mono text-base font-semibold tracking-tight text-ink"
          >
            <span className="inline-flex h-6 w-6 items-center justify-center rounded bg-primaryFill text-sm font-bold text-white">
              J
            </span>
            <span>Jev-Switch</span>
          </a>
          <nav aria-label="primary" className="flex items-center gap-1">
            {NAV.map((item) => {
              const active = item.route === route;
              return (
                <a
                  key={item.route}
                  href={item.href}
                  aria-current={active ? 'page' : undefined}
                  className={
                    'rounded-ctl border px-3 py-1.5 text-sm transition-colors ' +
                    (active
                      ? 'border-border bg-panel font-medium text-ink shadow-sm'
                      : 'border-transparent text-inkMuted hover:bg-soft hover:text-ink')
                  }
                >
                  {item.label}
                </a>
              );
            })}
          </nav>
        </div>
        {/* 顶栏第二行：daemon 状态胶囊 · masked-key hint · 版本 · 主题切换 */}
        <div className="border-t border-border">
          <div className="mx-auto flex max-w-7xl items-center justify-between gap-4 px-6 py-1.5 font-mono text-[10px] uppercase tracking-widest">
            <StatusBadge tone={daemonTone} aria-live="polite">
              {daemonLabel}
            </StatusBadge>
            <span className="flex items-center gap-3">
              <span className="text-inkMuted">masked-key</span>
              <span className="text-inkMuted tabular">v{pkg.version}</span>
              {/* 主题切换钮：mono 工具感文字钮（块 2） */}
              <button
                type="button"
                onClick={toggleTheme}
                aria-label={
                  theme === 'dark'
                    ? 'Switch to light theme (当前：深色)'
                    : 'Switch to dark theme (当前：浅色)'
                }
                aria-pressed={theme === 'dark'}
                className="inline-flex items-center gap-1 rounded-ctl border border-border bg-panel px-2 py-0.5 text-inkMuted transition-colors hover:border-primaryBright hover:bg-soft hover:text-primary"
              >
                <span aria-hidden>{theme === 'dark' ? '☾' : '☀'}</span>
                <span>{theme === 'dark' ? 'DARK' : 'LIGHT'}</span>
              </button>
            </span>
          </div>
        </div>
      </header>

      {/* 冲突横幅槽位（状态由页面 raise 接入） */}
      {conflict && handlers && (
        <ConflictBanner onReload={handlers.reload} onOverwrite={handlers.overwrite} />
      )}

      {/* 页面 */}
      <main className="flex-1">{children}</main>

      {/* footer endpoint 行（design/01 §5）— bg 底上用 muted 保 ≥4.5 */}
      <footer className="border-t border-border bg-bg">
        <div className="mx-auto max-w-7xl px-6 py-3 font-mono text-[10px] uppercase tracking-widest text-inkMuted">
          endpoint {endpointLabel} · /v1/systemone · file=truth
        </div>
      </footer>

      {/* toast 槽位 — B1 tint 胶囊 */}
      {toasts.length > 0 && (
        <div className="fixed bottom-4 right-4 z-50 flex w-80 flex-col gap-2" aria-live="polite">
          {toasts.map((t) => (
            <div
              key={t.id}
              className={
                'rounded-ctl border px-3 py-2 font-mono text-xs ' +
                (t.kind === 'ok'
                  ? 'border-transparent bg-okBg text-ok'
                  : t.kind === 'warn'
                    ? 'border-transparent bg-warnBg text-warn'
                    : 'border-transparent bg-dangerBg text-danger')
              }
            >
              {t.message}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
