import { useState } from 'react';
import type { AdminProvider, ProbeResponse } from '../../api/admin';
import { KeyForm } from './KeyForm';
import { ProbeButton } from './ProbeButton';
import { StatusBadge } from '../ui/StatusBadge';

interface Props {
  provider: AdminProvider;
  /** 该卡有 PUT 在途（toggle/key/delete） */
  busy: boolean;
  onToggle: (provider: AdminProvider, enabled: boolean) => void;
  onReplaceKey: (provider: AdminProvider, apiKey: string) => Promise<void>;
  onDelete: (provider: AdminProvider) => void;
}

/** 状态点语义（B1）：点用 500-600 档（非文本 ≥3:1），文字由 StatusBadge 700 档承载 */
function statusDotClass(provider: AdminProvider, probe: ProbeResponse | null): string {
  if (probe === null) return provider.enabled ? 'bg-inkSubtle' : 'bg-border';
  if (!probe.ok) return 'bg-dangerDot';
  if (provider.enabled && probe.latency_ms >= 1000) return 'bg-warnDot';
  return provider.enabled ? 'bg-okDot' : 'bg-inkSubtle';
}

function statusTone(provider: AdminProvider, probe: ProbeResponse | null): 'ok' | 'warn' | 'danger' | 'muted' {
  if (probe === null) return provider.enabled ? 'muted' : 'muted';
  if (!probe.ok) return 'danger';
  if (provider.enabled && probe.latency_ms >= 1000) return 'warn';
  return provider.enabled ? 'ok' : 'muted';
}

function statusLabel(provider: AdminProvider, probe: ProbeResponse | null): string {
  if (probe === null) return provider.enabled ? '未探测' : '已停用';
  if (!probe.ok) return 'probe 失败';
  if (provider.enabled && probe.latency_ms >= 1000) return '降级（慢）';
  return provider.enabled ? '健康' : '已停用';
}

/** 卡片状态高亮（B2）：启用且健康=蓝洗底 / 降级=琥珀描边 / 失败=红描边 */
function cardStateClass(provider: AdminProvider, probe: ProbeResponse | null): string {
  if (probe !== null && !probe.ok) return 'card-danger';
  if (probe !== null && provider.enabled && probe.latency_ms >= 1000) return 'card-warn';
  if (provider.enabled !== false && probe !== null && probe.ok) return 'card-active';
  return '';
}

/** probe 延迟火花线（design/01 §6.1 ▁▂▃ 可选位 · 图表 C 案）：失败=顶点红点，线走 primary 蓝 */
function Sparkline({ hist }: { hist: ReadonlyArray<{ ms: number; ok: boolean }> }) {
  if (hist.length < 2) {
    return (
      <span className="text-inkSubtle" aria-hidden>
        ▁▁▁
      </span>
    );
  }
  const w = 64;
  const h = 16;
  const max = Math.max(...hist.map((p) => (p.ok ? p.ms : 0)), 1);
  const step = w / Math.max(hist.length - 1, 1);
  const pts = hist.map((p, i) => {
    const x = Math.round(i * step);
    const y = p.ok ? h - 1 - Math.round((p.ms / max) * (h - 3)) : 1;
    return { ...p, x, y };
  });
  const fails = pts.filter((p) => !p.ok);
  return (
    <svg width={w} height={h} viewBox={`0 0 ${w} ${h}`} className="shrink-0" aria-hidden>
      <polyline
        points={pts.map((p) => `${p.x},${p.y}`).join(' ')}
        fill="none"
        stroke="var(--primary)"
        strokeWidth="1.5"
        strokeLinejoin="round"
        strokeLinecap="round"
      />
      {fails.map((p, i) => (
        <circle key={i} cx={p.x} cy={p.y} r="2" fill="var(--danger-fill)" />
      ))}
    </svg>
  );
}

/**
 * 提供商卡片（design/01 §6.1 × TeamSense 三段式 A1 × 方案 B 色）：
 * 头（id+状态徽章+Probe+开关）/ 身（kind/base/key 掩码）/ 脚（service-row 探测结果）。
 * 密钥只出掩码（契约 04 §2），卡片内不存在任何明文。
 */
export function ProviderCard({ provider, busy, onToggle, onReplaceKey, onDelete }: Props) {
  const [probe, setProbe] = useState<ProbeResponse | null>(null);
  const [hist, setHist] = useState<ReadonlyArray<{ ms: number; ok: boolean }>>([]);
  const [showKeyForm, setShowKeyForm] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deleteText, setDeleteText] = useState('');

  const onProbe = (r: ProbeResponse) => {
    setProbe(r);
    setHist((h) => [...h.slice(-23), { ms: r.latency_ms, ok: r.ok }]);
  };

  const dotClass = statusDotClass(provider, probe);
  const tone = statusTone(provider, probe);
  const label = statusLabel(provider, probe);
  const stateClass = cardStateClass(provider, probe);

  return (
    <section
      className={`flex flex-col overflow-hidden rounded-card border border-border bg-panel transition-colors ${stateClass}`.trim()}
      aria-label={`provider ${provider.id}`}
    >
      {/* 头：● id + 状态徽章 — Probe + ENABLED 开关 */}
      <header className="flex items-center justify-between gap-3 border-b border-border px-4 py-2.5">
        <span className="flex min-w-0 items-center gap-2.5" aria-live="polite">
          <span className={`inline-block h-1.5 w-1.5 shrink-0 rounded-full ${dotClass}`} aria-hidden />
          <span className="truncate text-[13px] font-semibold text-ink tabular">{provider.id}</span>
          <StatusBadge tone={tone}>{label}</StatusBadge>
        </span>
        <span className="flex shrink-0 items-center gap-3">
          <ProbeButton providerId={provider.id} onResult={onProbe} />
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
                'relative inline-block h-5 w-9 rounded border transition-colors ' +
                (busy ? 'opacity-50 ' : '') +
                (provider.enabled
                  ? 'border-primaryFill bg-primaryFill'
                  : 'border-border bg-soft')
              }
            >
              <span
                className={
                  'absolute top-0.5 h-3 w-3 rounded border border-transparent transition-all ' +
                  (provider.enabled ? 'left-[18px] bg-white' : 'left-0.5 bg-inkMuted')
                }
              />
            </span>
          </label>
        </span>
      </header>

      {/* 身：kind + base + masked key */}
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
            className="h-8 shrink-0 border border-border bg-panel px-2 font-mono text-xs text-ink hover:border-primaryBright hover:bg-soft disabled:opacity-50"
          >
            {showKeyForm ? 'Close' : 'Replace key'}
          </button>
        </div>
      </div>

      {/* 脚 = service-row（A4）：结果 ∥ 火花线 ∥ 时间 — aria-live（design/01 §8） */}
      <div
        className="grid grid-cols-[1fr_auto_auto] items-center gap-3 border-t border-border px-4 py-2 font-mono text-xs tabular"
        aria-live="polite"
      >
        {probe === null ? (
          <>
            <span className="text-inkMuted">last — · 未探测</span>
            <Sparkline hist={hist} />
            <span className="text-inkSubtle">—</span>
          </>
        ) : probe.ok ? (
          <>
            <span className="text-ok">
              last {probe.latency_ms}ms · {probe.status} ok
            </span>
            <Sparkline hist={hist} />
            <span className="text-inkSubtle">now</span>
          </>
        ) : (
          <>
            <span className="text-danger">
              probe failed{probe.status !== null ? ` · ${probe.status}` : ''} · {probe.error ?? 'error'}
            </span>
            <Sparkline hist={hist} />
            <span className="text-inkSubtle">now</span>
          </>
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
              className="h-8 w-36 border border-border bg-soft px-2 font-mono text-xs text-ink placeholder:text-inkSubtle"
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
              className="h-8 border border-dangerFill bg-dangerFill px-2 font-mono text-xs text-white hover:bg-dangerBg hover:text-danger disabled:cursor-not-allowed disabled:opacity-40"
            >
              Remove
            </button>
            <button
              type="button"
              onClick={() => setConfirmDelete(false)}
              className="h-8 border border-border bg-panel px-2 font-mono text-xs text-inkMuted hover:border-primaryBright hover:text-ink"
            >
              Cancel
            </button>
          </div>
        )}
      </div>
    </section>
  );
}
