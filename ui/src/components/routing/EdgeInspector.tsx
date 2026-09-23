import { useEffect, useState, type CSSProperties } from 'react';
import type { Route } from '../../api/admin';

interface Props {
  route: Route;
  style: CSSProperties;
  onPatch: (patch: Partial<Route>) => void;
  onDelete: () => void;
  onClose: () => void;
}

/**
 * 边检视器浮层（design/01 §6.2 / 契约 03 §6）：
 * priority / upstream_model / sticky / on_error 四字段 + Delete。
 * 仅密文无关 — 纯路由字段。
 */
export function EdgeInspector({ route, style, onPatch, onDelete, onClose }: Props) {
  const [prio, setPrio] = useState(String(route.priority));
  useEffect(() => {
    setPrio(String(route.priority));
  }, [route.priority]);

  const commitPrio = (v: string) => {
    setPrio(v);
    const n = Number(v);
    if (v.trim() !== '' && Number.isFinite(n)) onPatch({ priority: Math.trunc(n) });
  };

  return (
    <div
      className="absolute z-20 w-64 border border-ink bg-panel"
      style={style}
      onClick={(e) => e.stopPropagation()}
      role="dialog"
      aria-label={`edge ${route.left} to ${route.right}`}
    >
      <header className="flex items-center justify-between gap-2 border-b border-border px-3 py-2">
        <span className="min-w-0 truncate font-mono text-[11px] text-ink" title={`${route.left} → ${route.right}`}>
          {route.left} → {route.right}
        </span>
        <button
          type="button"
          onClick={onClose}
          aria-label="close inspector"
          className="shrink-0 px-1 font-mono text-xs text-inkSubtle hover:text-ink"
        >
          ×
        </button>
      </header>

      <div className="space-y-2.5 px-3 py-3">
        <Field label="priority">
          <input
            type="number"
            value={prio}
            onChange={(e) => commitPrio(e.target.value)}
            className="w-full border border-border bg-bg px-2 py-1 font-mono text-xs text-ink tabular"
            aria-label="priority"
          />
        </Field>
        <Field label="upstream_model" hint="空 = 沿用">
          <input
            type="text"
            value={route.upstream_model ?? ''}
            onChange={(e) => {
              const v = e.target.value;
              onPatch(v.length > 0 ? { upstream_model: v } : { upstream_model: undefined });
            }}
            placeholder="typesafe-ai/jev"
            spellCheck={false}
            className="w-full border border-border bg-bg px-2 py-1 font-mono text-xs text-ink placeholder:text-inkSubtle"
            aria-label="upstream model"
          />
        </Field>
        <Field label="sticky">
          <select
            value={route.sticky ?? 'none'}
            onChange={(e) => onPatch({ sticky: e.target.value as 'none' | 'session' })}
            className="w-full border border-border bg-bg px-2 py-1 font-mono text-xs text-ink"
            aria-label="sticky"
          >
            <option value="none">none</option>
            <option value="session">session</option>
          </select>
        </Field>
        <Field label="on_error">
          <select
            value={route.on_error ?? 'next'}
            onChange={(e) => onPatch({ on_error: e.target.value as 'next' | 'fail' })}
            className="w-full border border-border bg-bg px-2 py-1 font-mono text-xs text-ink"
            aria-label="on error"
          >
            <option value="next">next</option>
            <option value="fail">fail</option>
          </select>
        </Field>
      </div>

      <footer className="flex items-center justify-between border-t border-border px-3 py-2">
        <button
          type="button"
          onClick={onDelete}
          className="border border-danger bg-danger px-2 py-0.5 font-mono text-[11px] text-panel hover:bg-panel hover:text-danger"
        >
          Delete
        </button>
        <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
          Backspace 亦可删
        </span>
      </footer>
    </div>
  );
}

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="flex items-baseline justify-between">
        <span className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
          {label}
        </span>
        {hint && (
          <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
            {hint}
          </span>
        )}
      </span>
      {children}
    </label>
  );
}
