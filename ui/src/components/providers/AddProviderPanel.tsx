import { useState } from 'react';
import {
  parseProvidersToml,
  type AdminProviderWrite,
} from '../../api/admin';

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

/**
 * + Add provider 面板（design/01 §6.1）：表单 / 粘贴 toml 片段 双 Tab。
 * api_key 走 password 输入（无 Show）；解析错误全部回显，不静默半导入。
 */
export function AddProviderPanel({ initialTab = 'form', onAdd, onCancel }: Props) {
  const [tab, setTab] = useState<'form' | 'toml'>(initialTab);
  const [form, setForm] = useState<FormState>(EMPTY_FORM);
  const [toml, setToml] = useState('');
  const [busy, setBusy] = useState(false);
  const [errors, setErrors] = useState<string[]>([]);

  const tabClass = (active: boolean) =>
    'px-2.5 py-1 font-mono text-xs transition-colors ' +
    (active
      ? 'border border-ink bg-ink text-bg'
      : 'border border-transparent text-inkMuted hover:text-ink');

  const submitForm = async (e: React.FormEvent) => {
    e.preventDefault();
    const next: string[] = [];
    if (!form.id.trim()) next.push('id 必填');
    if (!form.kind.trim()) next.push('kind 必填');
    if (!/^https?:\/\/.+/.test(form.base.trim())) next.push('base 需以 http(s):// 开头');
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
        parseErrors.length > 0 ? parseErrors : ['未解析到任何 [providers.<id>] 段'],
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
    <section className="border border-border bg-panel">
      <header className="flex items-center justify-between border-b border-border px-4 py-2">
        <div className="flex items-center gap-1">
          <button type="button" className={tabClass(tab === 'form')} onClick={() => setTab('form')}>
            表单
          </button>
          <button type="button" className={tabClass(tab === 'toml')} onClick={() => setTab('toml')}>
            贴 toml 片段
          </button>
        </div>
        <button
          type="button"
          onClick={onCancel}
          className="font-mono text-[11px] text-inkSubtle hover:text-ink"
        >
          Close
        </button>
      </header>

      {errors.length > 0 && (
        <div className="border-b border-danger bg-danger/10 px-4 py-2" role="alert">
          {errors.map((err, i) => (
            <div key={i} className="font-mono text-[11px] text-danger">
              {err}
            </div>
          ))}
        </div>
      )}

      {tab === 'form' ? (
        <form onSubmit={(e) => void submitForm(e)} className="space-y-3 p-4">
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <Field label="id" hint="必填">
              <input
                value={form.id}
                onChange={(e) => setForm((f) => ({ ...f, id: e.target.value }))}
                placeholder="vercel"
                spellCheck={false}
                className="w-full border border-border bg-bg px-2 py-1.5 font-mono text-xs text-ink placeholder:text-inkSubtle"
              />
            </Field>
            <Field label="kind" hint="必填">
              <input
                value={form.kind}
                onChange={(e) => setForm((f) => ({ ...f, kind: e.target.value }))}
                placeholder="vercel-gateway"
                spellCheck={false}
                className="w-full border border-border bg-bg px-2 py-1.5 font-mono text-xs text-ink placeholder:text-inkSubtle"
              />
            </Field>
          </div>
          <Field label="base" hint="http(s)://…">
            <input
              value={form.base}
              onChange={(e) => setForm((f) => ({ ...f, base: e.target.value }))}
              placeholder="https://…"
              spellCheck={false}
              className="w-full border border-border bg-bg px-2 py-1.5 font-mono text-xs text-ink placeholder:text-inkSubtle"
            />
          </Field>
          <Field label="api_key" hint="仅密文输入 · 无 Show">
            <input
              type="password"
              value={form.apiKey}
              onChange={(e) => setForm((f) => ({ ...f, apiKey: e.target.value }))}
              placeholder="••••••••"
              autoComplete="new-password"
              spellCheck={false}
              className="w-full border border-border bg-bg px-2 py-1.5 font-mono text-xs text-ink placeholder:text-inkSubtle"
            />
          </Field>
          <label className="flex items-center gap-2 text-xs text-ink">
            <input
              type="checkbox"
              checked={form.enabled}
              onChange={(e) => setForm((f) => ({ ...f, enabled: e.target.checked }))}
              className="accent-ink"
            />
            enabled
          </label>
          <div className="flex gap-2 pt-1">
            <button
              type="submit"
              disabled={busy}
              className="border border-ink bg-ink px-3 py-1.5 font-mono text-xs text-bg hover:bg-bg hover:text-ink disabled:opacity-50"
            >
              {busy ? 'Saving…' : 'Add provider'}
            </button>
          </div>
        </form>
      ) : (
        <form onSubmit={(e) => void submitToml(e)} className="space-y-3 p-4">
          <div className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
            粘贴 [providers.&lt;id&gt;] 片段
          </div>
          <textarea
            value={toml}
            onChange={(e) => setToml(e.target.value)}
            rows={8}
            spellCheck={false}
            placeholder={'[providers.vercel]\nkind = "vercel-gateway"\nbase = "https://…"\napi_key = "…"\nenabled = true'}
            className="w-full resize-y border border-border bg-bg p-2.5 font-mono text-xs leading-relaxed text-ink placeholder:text-inkSubtle"
          />
          <div className="flex gap-2">
            <button
              type="submit"
              disabled={busy}
              className="border border-ink bg-ink px-3 py-1.5 font-mono text-xs text-bg hover:bg-bg hover:text-ink disabled:opacity-50"
            >
              {busy ? 'Saving…' : 'Parse & Add'}
            </button>
            <span className="self-center font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
              解析失败会全量回显 · 不半导入
            </span>
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
