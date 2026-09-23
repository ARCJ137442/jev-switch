import { useEffect, useState, type ReactNode } from 'react';
import { BASE, fetchHealth } from '../api';
import { ConflictBanner } from '../components/ConflictBanner';
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

/** design/01 §5 版式：顶栏两行 + 冲突横幅槽 + 页面 + endpoint footer 行 */
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

  return (
    <div className="flex min-h-full flex-col bg-bg text-ink">
      {/* 顶栏第一行：logo + 三页导航 */}
      <header className="border-b border-border bg-bg">
        <div className="mx-auto flex max-w-7xl items-center justify-between gap-6 px-6 py-3.5">
          <a
            href="#/playground"
            className="flex items-center gap-2 font-mono text-base font-semibold tracking-tight text-ink"
          >
            <span className="inline-flex h-6 w-6 items-center justify-center bg-ink text-sm font-bold text-bg">
              J
            </span>
            <span>Jev-Switch</span>
          </a>
          <nav aria-label="primary" className="flex items-center gap-4">
            {NAV.map((item) => {
              const active = item.route === route;
              return (
                <a
                  key={item.route}
                  href={item.href}
                  aria-current={active ? 'page' : undefined}
                  className={
                    'text-sm transition-colors ' +
                    (active ? 'font-medium text-ink' : 'text-inkMuted hover:text-ink')
                  }
                >
                  {item.label}
                </a>
              );
            })}
          </nav>
        </div>
        {/* 顶栏第二行：daemon 灯 · masked-key hint · 版本 */}
        <div className="border-t border-border">
          <div className="mx-auto flex max-w-7xl items-center justify-between gap-4 px-6 py-1.5 font-mono text-[10px] uppercase tracking-widest">
            <span className="flex items-center gap-1.5" aria-live="polite">
              <span
                className={
                  'inline-block h-1.5 w-1.5 rounded-full ' +
                  (server.status === 'ok'
                    ? 'bg-ok'
                    : server.status === 'error'
                      ? 'bg-danger'
                      : 'bg-inkSubtle')
                }
                aria-hidden
              />
              <span className="text-inkMuted">
                {server.status === 'ok'
                  ? 'daemon ok'
                  : server.status === 'error'
                    ? 'daemon unreachable'
                    : 'connecting…'}
              </span>
            </span>
            <span className="text-inkSubtle">masked-key</span>
            <span className="text-inkSubtle tabular">v{pkg.version}</span>
          </div>
        </div>
      </header>

      {/* 冲突横幅槽位（状态由页面 raise 接入） */}
      {conflict && handlers && (
        <ConflictBanner onReload={handlers.reload} onOverwrite={handlers.overwrite} />
      )}

      {/* 页面 */}
      <main className="flex-1">{children}</main>

      {/* footer endpoint 行（design/01 §5） */}
      <footer className="border-t border-border bg-bg">
        <div className="mx-auto max-w-7xl px-6 py-3 font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
          endpoint {endpointLabel} · /v1/systemone · file=truth
        </div>
      </footer>

      {/* toast 槽位 */}
      {toasts.length > 0 && (
        <div className="fixed bottom-4 right-4 z-50 flex w-80 flex-col gap-2" aria-live="polite">
          {toasts.map((t) => (
            <div
              key={t.id}
              className={
                'border bg-panel px-3 py-2 font-mono text-xs ' +
                (t.kind === 'ok'
                  ? 'border-ok text-ok'
                  : t.kind === 'warn'
                    ? 'border-warn text-warn'
                    : 'border-danger text-danger')
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
