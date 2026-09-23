import { useState } from 'react';
import {
  parseProvidersToml,
  type AdminProviderWrite,
} from '../../api/admin';
import { useI18n } from '../../i18n';

interface Props {
  /** toml | form — 空态引导可预选 toml */
  initialTab?: 'form' | 'toml';
  onAdd: (provider: AdminProviderWrite) => Promise<void>;
  onCancel: () => void;
}

interface FormState {
  id: string;
  kind: string;
  base: string;
  enabled: boolean;
  apiKey: string;
}

const EMPTY_FORM: FormState = { id: '', kind: '', base: '', enabled: true, apiKey: '' };

/** 输入框（v2：mono 仅用于 id/base/key 这类机读值） */
const inputStyle: React.CSSProperties = {
  fontFamily: 'var(--font-mono)',
  fontSize: 'var(--text-sm)',
  background: 'var(--surface-hover)',
  border: '1px solid var(--border)',
  borderRadius: 'var(--radius)',
  color: 'var(--text)',
};

const btn: React.CSSProperties = {
  fontSize: 'var(--text-sm)',
  background: 'var(--surface)',
  border: '1px solid var(--border)',
  borderRadius: 'var(--radius)',
  color: 'var(--text)',
  padding: '0.4rem 0.8rem',
};

const btnPrimary: React.CSSProperties = {
  ...btn,
  background: 'var(--accent)',
  borderColor: 'var(--accent)',
  color: '#fff',
  fontWeight: 600,
};

/**
 * + Add provider 面板（v2 设计系统）：表单 / 粘贴 toml 片段 双 Tab。
 * api_key 走 password 输入（无 Show）；解析错误全部回显，不静默半导入。
 */
export function AddProviderPanel({ initialTab = 'form', onAdd, onCancel }: Props) {
  const { t } = useI18n();
  const [tab, setTab] = useState<'form' | 'toml'>(initialTab);
  const [form, setForm] = useState<FormState>(EMPTY_FORM);
  const [toml, setToml] = useState('');
  const [busy, setBusy] = useState(false);
  const [errors, setErrors] = useState<string[]>([]);

  const tabStyle = (active: boolean): React.CSSProperties => ({
    fontSize: 'var(--text-sm)',
    fontWeight: active ? 600 : 400,
    padding: '0.4rem 0.8rem',
    borderRadius: 'var(--radius)',
    border: '1px solid',
    borderColor: active ? 'var(--accent)' : 'transparent',
    background: active ? 'var(--accent)' : 'transparent',
    color: active ? '#fff' : 'var(--text-muted)',
  });

  const submitForm = async (e: React.FormEvent) => {
    e.preventDefault();
    const next: string[] = [];
    if (!form.id.trim()) next.push(t('add.idRequired'));
    if (!form.kind.trim()) next.push(t('add.kindRequired'));
    if (!/^https?:\/\/.+/.test(form.base.trim())) next.push(t('add.baseHttp'));
    if (next.length > 0) {
      setErrors(next);
      return;
    }
    setErrors([]);
    setBusy(true);
    try {
      const provider: AdminProviderWrite = {
        id: form.id.trim(),
        kind: form.kind.trim(),
        base: form.base.trim(),
        enabled: form.enabled,
      };
      if (form.apiKey.length > 0) provider.api_key = form.apiKey;
      await onAdd(provider);
      setForm(EMPTY_FORM);
    } catch (e2) {
      setErrors([(e2 as Error).message]);
    } finally {
      setBusy(false);
    }
  };

  const submitToml = async (e: React.FormEvent) => {
    e.preventDefault();
    const { providers, errors: parseErrors } = parseProvidersToml(toml);
    if (parseErrors.length > 0 || providers.length === 0) {
      setErrors(
        parseErrors.length > 0 ? parseErrors : [t('add.noSections')],
      );
      return;
    }
    setErrors([]);
    setBusy(true);
    try {
      // 逐条写入（PUT 整表语义由页面拼接；多段按顺序追加）
      for (const p of providers) {
        await onAdd(p);
      }
      setToml('');
    } catch (e2) {
      setErrors([(e2 as Error).message]);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section
      className="fade-in overflow-hidden"
      style={{
        background: 'var(--surface)',
        border: '1px solid var(--border)',
        borderRadius: 'var(--radius)',
      }}
    >
      <header
        className="flex items-center justify-between px-4 py-2.5"
        style={{ borderBottom: '1px solid var(--border)' }}
      >
        <div className="flex items-center gap-1">
          <button type="button" style={tabStyle(tab === 'form')} onClick={() => setTab('form')}>
            {t('add.tabForm')}
          </button>
          <button
            type="button"
            style={tabStyle(tab === 'toml')}
            onClick={() => setTab('toml')}
            title={t('add.pasteHint')}
          >
            {t('prov.pasteToml')}
          </button>
        </div>
        <button
          type="button"
          onClick={onCancel}
          style={{ ...btn, background: 'transparent', borderColor: 'transparent', color: 'var(--text-muted)' }}
        >
          {t('common.close')}
        </button>
      </header>

      {errors.length > 0 && (
        <div
          className="px-4 py-2.5"
          style={{ borderBottom: '1px solid var(--danger)', background: 'var(--danger-bg)' }}
          role="alert"
        >
          {errors.map((err, i) => (
            <div key={i} style={{ fontSize: 'var(--text-sm)', color: 'var(--danger)' }}>
              {err}
            </div>
          ))}
        </div>
      )}

      {tab === 'form' ? (
        <form onSubmit={(e) => void submitForm(e)} className="space-y-3 px-4 py-4">
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <Field label="id" hint={t('common.required')}>
              <input
                value={form.id}
                onChange={(e) => setForm((f) => ({ ...f, id: e.target.value }))}
                placeholder="vercel"
                spellCheck={false}
                className="h-9 w-full px-2.5"
                style={inputStyle}
              />
            </Field>
            <Field label="kind" hint={t('common.required')}>
              <input
                value={form.kind}
                onChange={(e) => setForm((f) => ({ ...f, kind: e.target.value }))}
                placeholder="vercel-gateway"
                spellCheck={false}
                className="h-9 w-full px-2.5"
                style={inputStyle}
              />
            </Field>
          </div>
          <Field label="base" hint="http(s)://…">
            <input
              value={form.base}
              onChange={(e) => setForm((f) => ({ ...f, base: e.target.value }))}
              placeholder="https://…"
              spellCheck={false}
              className="h-9 w-full px-2.5"
              style={inputStyle}
            />
          </Field>
          <Field label="api_key">
            <input
              type="password"
              value={form.apiKey}
              onChange={(e) => setForm((f) => ({ ...f, apiKey: e.target.value }))}
              placeholder="••••••••"
              autoComplete="new-password"
              spellCheck={false}
              title={t('add.apiKeyHint')}
              className="h-9 w-full px-2.5"
              style={inputStyle}
            />
          </Field>
          <label
            className="flex items-center gap-2"
            style={{ fontSize: 'var(--text-sm)', color: 'var(--text)' }}
          >
            <input
              type="checkbox"
              checked={form.enabled}
              onChange={(e) => setForm((f) => ({ ...f, enabled: e.target.checked }))}
              style={{ accentColor: 'var(--accent)' }}
            />
            {t('add.enabled')}
          </label>
          <div className="flex gap-2 pt-1">
            <button
              type="submit"
              disabled={busy}
              className="disabled:opacity-50"
              style={btnPrimary}
            >
              {busy ? t('common.saving') : t('add.submit')}
            </button>
          </div>
        </form>
      ) : (
        <form onSubmit={(e) => void submitToml(e)} className="space-y-3 px-4 py-4">
          <textarea
            value={toml}
            onChange={(e) => setToml(e.target.value)}
            rows={8}
            spellCheck={false}
            title={t('add.parseHint')}
            aria-label={t('add.pasteHint')}
            placeholder={'[providers.vercel]\nkind = "vercel-gateway"\nbase = "https://…"\napi_key = "…"\nenabled = true'}
            className="w-full resize-y p-3 leading-relaxed"
            style={inputStyle}
          />
          <div className="flex gap-2">
            <button
              type="submit"
              disabled={busy}
              className="disabled:opacity-50"
              style={btnPrimary}
              title={t('add.parseHint')}
            >
              {busy ? t('common.saving') : t('add.parseAdd')}
            </button>
          </div>
        </form>
      )}
    </section>
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
    <label className="flex flex-col gap-1.5">
      <span className="flex items-baseline justify-between gap-2">
        <span
          style={{
            fontFamily: 'var(--font-mono)',
            fontSize: 'var(--text-sm)',
            fontWeight: 600,
            color: 'var(--text)',
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
