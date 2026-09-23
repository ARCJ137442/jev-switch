import { useState } from 'react';
import type { AdminProvider, ProbeResponse } from '../../api/admin';
import { KeyForm } from './KeyForm';
import { ProbeButton } from './ProbeButton';

interface Props {
  provider: AdminProvider;
  /** 该卡有 PUT 在途（toggle/key/delete） */
  busy: boolean;
  onToggle: (provider: AdminProvider, enabled: boolean) => void;
  onReplaceKey: (provider: AdminProvider, apiKey: string) => Promise<void>;
  onDelete: (provider: AdminProvider) => void;
}

/** 状态点语义映射（design/01 §6.1）：probe 结果 → ok/warn/danger，未测/disabled → subtle */
function statusDotClass(provider: AdminProvider, probe: ProbeResponse | null): string {
  if (probe === null) return provider.enabled ? 'bg-inkSubtle' : 'bg-border';
  if (!probe.ok) return 'bg-danger';
  if (provider.enabled && probe.latency_ms >= 1000) return 'bg-warn';
  return provider.enabled ? 'bg-ok' : 'bg-inkSubtle';
}

function statusDotLabel(provider: AdminProvider, probe: ProbeResponse | null): string {
  if (probe === null) return provider.enabled ? '未探测' : '已停用';
  if (!probe.ok) return 'probe 失败';
  if (provider.enabled && probe.latency_ms >= 1000) return '降级（慢）';
  return provider.enabled ? '健康' : '已停用';
}

/**
 * 提供商卡片（design/01 §6.1）：
 * 状态点 / id+kind+base / masked key + Replace key / Probe + footer 结果 / ENABLED 开关 / 打字 id 删除。
 * 密钥只出掩码（契约 04 §2），卡片内不存在任何明文。
 */
export function ProviderCard({ provider, busy, onToggle, onReplaceKey, onDelete }: Props) {
  const [probe, setProbe] = useState<ProbeResponse | null>(null);
  const [showKeyForm, setShowKeyForm] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deleteText, setDeleteText] = useState('');

  const dotClass = statusDotClass(provider, probe);
  const dotLabel = statusDotLabel(provider, probe);

  return (
    <section
      className="flex flex-col border border-border bg-panel"
      aria-label={`provider ${provider.id}`}
    >
      {/* header: ● id — Probe + ENABLED 开关 */}
      <header className="flex items-center justify-between gap-3 border-b border-border px-4 py-2.5">
        <span className="flex min-w-0 items-center gap-2.5" aria-live="polite">
          <span className={`inline-block h-1.5 w-1.5 shrink-0 rounded-full ${dotClass}`} aria-hidden />
          <span className="truncate text-[13px] font-semibold text-ink tabular">{provider.id}</span>
          <span className="sr-only">{dotLabel}</span>
        </span>
        <span className="flex shrink-0 items-center gap-3">
          <ProbeButton providerId={provider.id} onResult={setProbe} />
          <label className="flex cursor-pointer items-center gap-1.5">
            <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
              {provider.enabled ? 'ENABLED' : 'OFF'}
            </span>
            <span
              role="switch"
              aria-checked={provider.enabled}
              aria-label={`enable ${provider.id}`}
              tabIndex={0}
              onClick={() => !busy && onToggle(provider, !provider.enabled)}
              onKeyDown={(e) => {
                if (e.key === ' ' || e.key === 'Enter') {
                  e.preventDefault();
                  if (!busy) onToggle(provider, !provider.enabled);
                }
              }}
              className={
                'relative inline-block h-5 w-9 border transition-colors ' +
                (busy ? 'opacity-50 ' : '') +
                (provider.enabled ? 'border-ink bg-ink' : 'border-inkSubtle bg-bg')
              }
            >
              <span
                className={
                  'absolute top-0.5 h-3 w-3 transition-all ' +
                  (provider.enabled ? 'left-[18px] bg-bg' : 'left-0.5 bg-ink')
                }
              />
            </span>
          </label>
        </span>
      </header>

      {/* kind + base */}
      <div className="flex-1 space-y-1 px-4 py-3">
        <div className="font-mono text-xs text-inkMuted">{provider.kind}</div>
        <div className="truncate font-mono text-xs text-inkSubtle" title={provider.base}>
          {provider.base}
        </div>

        {/* key 行 — 只出掩码，无 Show 明文 */}
        <div className="mt-2 flex items-center justify-between gap-2">
          <span className="flex min-w-0 items-baseline gap-2 font-mono text-xs">
            <span className="text-inkSubtle">key</span>
            <span className="truncate text-ink tabular">
              {provider.api_key_set ? (provider.api_key_masked ?? '****') : '(unset)'}
            </span>
          </span>
          <button
            type="button"
            onClick={() => {
              setShowKeyForm((v) => !v);
              setConfirmDelete(false);
            }}
            disabled={busy}
            className="h-8 shrink-0 border border-border bg-panel px-2 font-mono text-xs text-ink hover:border-ink disabled:opacity-50"
          >
            {showKeyForm ? 'Close' : 'Replace key'}
          </button>
        </div>
      </div>

      {/* probe footer — aria-live 结果（design/01 §8） */}
      <div
        className="border-t border-border px-4 py-2 font-mono text-xs tabular"
        aria-live="polite"
      >
        {probe === null ? (
          <span className="text-inkSubtle">last — · 未探测</span>
        ) : probe.ok ? (
          <span className="text-ok">
            last {probe.latency_ms}ms · {probe.status} ok
          </span>
        ) : (
          <span className="text-danger">
            probe failed{probe.status !== null ? ` · ${probe.status}` : ''} · {probe.error ?? 'error'}
          </span>
        )}
      </div>

      {/* Replace key 内联表单（仅密码式） */}
      {showKeyForm && (
        <KeyForm
          onSave={(key) => onReplaceKey(provider, key)}
          onCancel={() => setShowKeyForm(false)}
        />
      )}

      {/* 删除：打字 id 二次确认 */}
      <div className="border-t border-border px-4 py-2.5">
        {!confirmDelete ? (
          <button
            type="button"
            onClick={() => {
              setConfirmDelete(true);
              setShowKeyForm(false);
              setDeleteText('');
            }}
            disabled={busy}
            className="h-8 font-mono text-xs text-inkSubtle hover:text-danger disabled:opacity-50"
          >
            Delete
          </button>
        ) : (
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
              type id to confirm
            </span>
            <input
              value={deleteText}
              onChange={(e) => setDeleteText(e.target.value)}
              placeholder={provider.id}
              spellCheck={false}
              autoFocus
              className="h-8 w-36 border border-border bg-bg px-2 font-mono text-xs text-ink placeholder:text-inkSubtle"
              aria-label={`type ${provider.id} to confirm delete`}
            />
            <button
              type="button"
              disabled={deleteText !== provider.id || busy}
              onClick={() => {
                onDelete(provider);
                setConfirmDelete(false);
                setDeleteText('');
              }}
              className="h-8 border border-danger bg-danger px-2 font-mono text-xs text-panel hover:bg-panel hover:text-danger disabled:cursor-not-allowed disabled:opacity-40"
            >
              Remove
            </button>
            <button
              type="button"
              onClick={() => setConfirmDelete(false)}
              className="h-8 border border-border bg-panel px-2 font-mono text-xs text-inkMuted hover:text-ink"
            >
              Cancel
            </button>
          </div>
        )}
      </div>
    </section>
  );
}
