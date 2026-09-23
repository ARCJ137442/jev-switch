import { useEffect, useState, type CSSProperties } from 'react';
import type { Route } from '../../api/admin';
import { useI18n } from '../../i18n';

interface Props {
  route: Route;
  style: CSSProperties;
  onPatch: (patch: Partial<Route>) => void;
  onDelete: () => void;
  onClose: () => void;
}

/**
 * 边检视器浮层（design/01 §6.2 / 契约 03 §6 × TeamSense Dialog 圆角）：
 * priority / upstream_model / sticky / on_error 四字段 + Delete。
 * 选中面板描边 = 主蓝（CC Switch border-active 手法）。
 */
export function EdgeInspector({ route, style, onPatch, onDelete, onClose }: Props) {
  const { t } = useI18n();
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
      className="absolute z-20 w-64 overflow-hidden rounded-card border border-primary bg-panel shadow-md"
      style={style}
      onClick={(e) => e.stopPropagation()}
      role="dialog"
      aria-label={`edge ${route.left} to ${route.right}`}
    >
      <header className="flex items-center justify-between gap-2 border-b border-border px-3 py-2">
        <span className="min-w-0 truncate font-mono text-xs text-ink tabular" title={`${route.left} → ${route.right}`}>
          {route.left} → {route.right}
        </span>
        <button
          type="button"
          onClick={onClose}
          aria-label="close inspector"
          className="inline-flex h-8 w-8 shrink-0 items-center justify-center font-mono text-xs text-inkSubtle hover:text-ink"
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
            className="h-8 w-full border border-border bg-soft px-2 font-mono text-xs text-ink tabular"
            aria-label="priority"
          />
        </Field>
        <Field label="upstream_model" hint={t('edge.hintKeep')}>
          <input
            type="text"
            value={route.upstream_model ?? ''}
            onChange={(e) => {
              const v = e.target.value;
              onPatch(v.length > 0 ? { upstream_model: v } : { upstream_model: undefined });
            }}
            placeholder="typesafe-ai/jev"
            spellCheck={false}
            className="h-8 w-full border border-border bg-soft px-2 font-mono text-xs text-ink placeholder:text-inkSubtle"
            aria-label="upstream model"
          />
        </Field>
        <Field label="sticky">
          <select
            value={route.sticky ?? 'none'}
            onChange={(e) => onPatch({ sticky: e.target.value as 'none' | 'session' })}
            className="h-8 w-full border border-border bg-soft px-2 font-mono text-xs text-ink"
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
            className="h-8 w-full border border-border bg-soft px-2 font-mono text-xs text-ink"
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
          className="h-8 border border-dangerFill bg-dangerFill px-2 font-mono text-xs text-white hover:bg-dangerBg hover:text-danger"
        >
          {t('common.delete')}
        </button>
        <span className="font-mono text-[10px] uppercase tracking-widest text-inkMuted">
          {t('edge.backspaceHint')}
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
        <span className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkMuted">
          {label}
        </span>
        {hint && (
          <span className="font-mono text-[10px] uppercase tracking-widest text-inkMuted">
            {hint}
          </span>
        )}
      </span>
      {children}
    </label>
  );
}
