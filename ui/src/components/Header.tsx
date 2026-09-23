import { useEffect, useState } from 'react';
import { fetchHealth } from '../api';

interface Props {
  serverStatus: 'unknown' | 'ok' | 'error';
  modelsCount: number;
  providersEnabled: number;
  providersTotal: number;
}

/**
 * 顶部 header — Logo + 站名 + 水平导航 + 服务器状态
 * 极简风 — 等宽 logo + 顶部细 border 分隔
 */
export function Header({ serverStatus, modelsCount, providersEnabled, providersTotal }: Props) {
  return (
    <header className="border-b border-border bg-bg">
      <div className="mx-auto flex max-w-7xl items-center justify-between px-6 py-4">
        <div className="flex items-center gap-6">
          <a href="#" className="flex items-center gap-2 font-mono text-base font-semibold tracking-tight text-ink">
            <span className="inline-flex h-6 w-6 items-center justify-center bg-ink text-bg text-sm font-bold">
              J
            </span>
            <span>Jev-Switch</span>
          </a>
          <nav className="hidden items-center gap-5 md:flex">
            <a href="#playground" className="text-sm text-ink hover:font-medium">
              Playground
            </a>
            <a href="#docs" className="text-sm text-inkMuted hover:text-ink">
              Docs
            </a>
            <a href="https://github.com/ARCJ137442" target="_blank" rel="noreferrer" className="text-sm text-inkMuted hover:text-ink">
              GitHub
            </a>
          </nav>
        </div>

        <div className="flex items-center gap-4 font-mono text-xs">
          <Stat label="models" value={modelsCount} />
          <span className="text-inkSubtle">·</span>
          <Stat label="providers" value={`${providersEnabled}/${providersTotal}`} />
          <span className="text-inkSubtle">·</span>
          <span className="flex items-center gap-1.5">
            <span
              className={
                'inline-block h-1.5 w-1.5 rounded-full ' +
                (serverStatus === 'ok'
                  ? 'bg-emerald-500'
                  : serverStatus === 'error'
                    ? 'bg-red-500'
                    : 'bg-inkSubtle')
              }
            />
            <span className="text-inkMuted">
              {serverStatus === 'ok'
                ? '8765 ok'
                : serverStatus === 'error'
                  ? '8765 down'
                  : 'connecting…'}
            </span>
          </span>
        </div>
      </div>
    </header>
  );
}

function Stat({ label, value }: { label: string; value: number | string }) {
  return (
    <span className="flex items-baseline gap-1">
      <span className="text-ink tabular">{value}</span>
      <span className="text-inkSubtle">{label}</span>
    </span>
  );
}

/**
 * useServerStatus — 检测后端 health。
 * 复用 health check 副作用，避免每个组件各自 fetch。
 */
export function useServerStatus(): {
  status: 'unknown' | 'ok' | 'error';
  text: string;
} {
  const [status, setStatus] = useState<'unknown' | 'ok' | 'error'>('unknown');
  const [text, setText] = useState<string>('');

  useEffect(() => {
    let cancelled = false;
    const probe = async () => {
      try {
        const t = await fetchHealth();
        if (!cancelled) {
          setStatus('ok');
          setText(t);
        }
      } catch (e) {
        if (!cancelled) {
          setStatus('error');
          setText((e as Error).message);
        }
      }
    };
    probe();
    const t = window.setInterval(probe, 8000);
    return () => {
      cancelled = true;
      window.clearInterval(t);
    };
  }, []);

  return { status, text };
}
