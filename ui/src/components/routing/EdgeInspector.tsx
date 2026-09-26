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

/** 表单控件（mono 仅用于 id / 数字值） */
const control: CSSProperties = {
  height: '2rem',
  width: '100%',
  border: '1px solid var(--border)',
  borderRadius: 'var(--radius)',
  background: 'var(--surface-hover)',
  padding: '0 0.5rem',
  fontSize: 'var(--text-sm)',
  color: 'var(--text)',
};

/**
 * 边检视器浮层（design/01 §6.2 / 契约 03 §6）：
 * priority / upstream_model / sticky / on_error 四字段 + Delete。
 * 选中面板描边 = 主色；键盘 Backspace/Delete 提示改为 Delete 按钮 title（渐进披露）。
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
      className="absolute z-20 w-64 overflow-hidden"
      style={{
        background: 'var(--surface)',
        border: '1px solid var(--accent)',
        borderRadius: 'var(--radius)',
        boxShadow: 'var(--shadow-lg)',
        ...style,
      }}
      onClick={(e) => e.stopPropagation()}
      role="dialog"
      aria-label={t('dag.edgeLabel', { left: route.left, right: route.right })}
    >
      <header
        className="flex items-center justify-between gap-2 px-3 py-2"
        style={{ borderBottom: '1px solid var(--border)' }}
      >
        <span
          className="min-w-0 truncate tabular"
          style={{
            fontFamily: 'var(--font-mono)',
            fontSize: 'var(--text-sm)',
            color: 'var(--text)',
          }}
          title={`${route.left} → ${route.right}`}
        >
          {route.left} → {route.right}
        </span>
        <button
          type="button"
          onClick={onClose}
          aria-label={t('common.close')}
          className="inline-flex h-8 w-8 shrink-0 items-center justify-center"
          style={{
            fontSize: 'var(--text-base)',
            color: 'var(--text-subtle)',
            background: 'transparent',
          }}
        >
          ×
        </button>
      </header>

      <div className="space-y-2.5 px-3 py-3">
        <Field label={t('entry.priority')}>
          <input
            type="number"
            value={prio}
            onChange={(e) => commitPrio(e.target.value)}
            className="tabular"
            style={{ ...control, fontFamily: 'var(--font-mono)' }}
            aria-label={t('entry.priority')}
          />
        </Field>
        <Field label={t('entry.upstreamModel')} hint={t('edge.hintKeep')}>
          <input
            type="text"
            value={route.upstream_model ?? ''}
            onChange={(e) => {
              const v = e.target.value;
              onPatch(v.length > 0 ? { upstream_model: v } : { upstream_model: undefined });
            }}
            placeholder="typesafe-ai/jev"
            spellCheck={false}
            style={{ ...control, fontFamily: 'var(--font-mono)' }}
            aria-label={t('entry.upstreamModel')}
          />
        </Field>
        <Field label={t('entry.sticky')}>
          <select
            value={route.sticky ?? 'none'}
            onChange={(e) => onPatch({ sticky: e.target.value as 'none' | 'session' })}
            style={control}
            aria-label={t('entry.sticky')}
          >
            <option value="none">{t('dag.none')}</option>
            <option value="session">{t('dag.session')}</option>
          </select>
        </Field>
        <Field label={t('entry.onError')}>
          <select
            value={route.on_error ?? 'next'}
            onChange={(e) => onPatch({ on_error: e.target.value as 'next' | 'fail' })}
            style={control}
            aria-label={t('entry.onError')}
          >
            <option value="next">{t('entry.next')}</option>
            <option value="fail">{t('entry.fail')}</option>
          </select>
        </Field>
      </div>

      <footer
        className="flex items-center justify-end px-3 py-2"
        style={{ borderTop: '1px solid var(--border)' }}
      >
        {/* 键盘删除提示改为 title（原常驻文字已删） */}
        <button
          type="button"
          onClick={onDelete}
          title={t('edge.backspaceHint')}
          style={{
            height: '2rem',
            padding: '0 0.75rem',
            fontSize: 'var(--text-sm)',
            fontWeight: 500,
            border: '1px solid var(--danger)',
            borderRadius: 'var(--radius)',
            background: 'var(--danger)',
            color: '#fff',
          }}
        >
          {t('common.delete')}
        </button>
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
      <span className="flex items-baseline justify-between gap-2">
        <span
          style={{
            fontSize: 'var(--text-xs)',
            fontWeight: 600,
            color: 'var(--text-muted)',
          }}
        >
          {label}
        </span>
        {hint && (
          <span style={{ fontSize: 'var(--text-xs)', color: 'var(--text-subtle)' }}>{hint}</span>
        )}
      </span>
      {children}
    </label>
  );
}
