import { useState } from 'react';
import type { AdminProvider, ProbeResponse } from '../../api/admin';
import { KeyForm } from './KeyForm';
import { ProbeButton } from './ProbeButton';
import { useI18n, type MessageKey } from '../../i18n';

interface Props {
  provider: AdminProvider;
  /** 该卡有 PUT 在途（toggle/key/delete） */
  busy: boolean;
  onToggle: (provider: AdminProvider, enabled: boolean) => void;
  onReplaceKey: (provider: AdminProvider, apiKey: string) => Promise<void>;
  onDelete: (provider: AdminProvider) => void;
}

type Health = 'ok' | 'warn' | 'danger' | 'off' | 'unknown';

/** 健康判定（v2 §3.2 健康灯前置）：probe 结果 × enabled → 一个状态 */
function healthOf(provider: AdminProvider, probe: ProbeResponse | null): Health {
  if (!provider.enabled) return 'off';
  if (probe === null) return 'unknown';
  if (!probe.ok) return 'danger';
  return probe.latency_ms >= 1000 ? 'warn' : 'ok';
}

const HEALTH_COLOR: Record<Health, string> = {
  ok: 'var(--success)',
  warn: 'var(--warning)',
  danger: 'var(--danger)',
  off: 'var(--text-subtle)',
  unknown: 'var(--text-subtle)',
};

/** 不健康时脉动（tokens.css .status-warning/.status-danger） */
const HEALTH_PULSE: Record<Health, string> = {
  ok: '',
  warn: 'status-warning',
  danger: 'status-danger',
  off: '',
  unknown: '',
};

const HEALTH_LABEL: Record<Health, MessageKey> = {
  ok: 'card.healthy',
  warn: 'card.degraded',
  danger: 'card.probeFailed',
  off: 'card.disabled',
  unknown: 'card.untested',
};

/** probe 延迟火花线：线走 --accent，失败点 --danger（v2 图形优先） */
function Sparkline({ hist }: { hist: ReadonlyArray<{ ms: number; ok: boolean }> }) {
  const w = 64;
  const h = 16;
  if (hist.length < 2) {
    return <span style={{ width: w, display: 'inline-block' }} aria-hidden />;
  }
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
        stroke="var(--accent)"
        strokeWidth="1.5"
        strokeLinejoin="round"
        strokeLinecap="round"
      />
      {fails.map((p, i) => (
        <circle key={i} cx={p.x} cy={p.y} r="2" fill="var(--danger)" />
      ))}
    </svg>
  );
}

/**
 * 提供商卡片（v2 设计系统 · docs/design/UI-REDESIGN-v2.md §3.2）：
 * 头「● Healthy + id」+ Probe + 开关 / 身 kind·base·掩码 key / 脚 探测结果 + 火花线。
 * 密钥只出掩码（契约 04 §2），卡片内不存在任何明文；说明文字全部走 title tooltip。
 */
export function ProviderCard({ provider, busy, onToggle, onReplaceKey, onDelete }: Props) {
  const [probe, setProbe] = useState<ProbeResponse | null>(null);
  const [hist, setHist] = useState<ReadonlyArray<{ ms: number; ok: boolean }>>([]);
  const [showKeyForm, setShowKeyForm] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const onProbe = (r: ProbeResponse) => {
    setProbe(r);
    setHist((h) => [...h.slice(-23), { ms: r.latency_ms, ok: r.ok }]);
  };

  const { t } = useI18n();
  const health = healthOf(provider, probe);
  const label = t(HEALTH_LABEL[health]);

  const card: React.CSSProperties = {
    background: 'var(--surface)',
    border: '1px solid var(--border)',
    borderRadius: 'var(--radius)',
  };
  const rowBorder: React.CSSProperties = { borderTop: '1px solid var(--border)' };
  const mono: React.CSSProperties = { fontFamily: 'var(--font-mono)' };
  const btn: React.CSSProperties = {
    fontSize: 'var(--text-sm)',
    background: 'var(--surface)',
    border: '1px solid var(--border)',
    borderRadius: 'var(--radius)',
    color: 'var(--text)',
    padding: '0.35rem 0.7rem',
  };

  return (
    <section
      className="fade-in card-hover flex flex-col overflow-hidden"
      style={card}
      aria-label={`provider ${provider.id}`}
    >
      {/* 头：● Healthy · id — Probe + 开关 */}
      <header
        className="flex items-center justify-between gap-3 px-4 py-3"
        style={{ borderBottom: '1px solid var(--border)' }}
      >
        <span className="flex min-w-0 items-center gap-2.5" aria-live="polite">
          <span
            className={HEALTH_PULSE[health]}
            style={{
              width: 10,
              height: 10,
              borderRadius: '50%',
              background: HEALTH_COLOR[health],
              display: 'inline-block',
              flexShrink: 0,
            }}
            aria-hidden
          />
          <span
            className="font-semibold"
            style={{ fontSize: 'var(--text-base)', color: HEALTH_COLOR[health] }}
          >
            {label}
          </span>
          <span
            className="tabular truncate"
            style={{ ...mono, fontSize: 'var(--text-sm)', color: 'var(--text-muted)' }}
            title={provider.id}
          >
            {provider.id}
          </span>
        </span>
        <span className="flex shrink-0 items-center gap-2.5">
          <ProbeButton providerId={provider.id} onResult={onProbe} />
          <label
            className="flex cursor-pointer items-center gap-1.5"
            title={t('card.toggleTip')}
          >
            <span style={{ fontSize: 'var(--text-xs)', color: 'var(--text-muted)' }}>
              {provider.enabled ? t('card.on') : t('card.off')}
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
              className="relative inline-block h-5 w-9 transition-colors"
              style={{
                borderRadius: 999,
                border: '1px solid',
                borderColor: provider.enabled ? 'var(--accent)' : 'var(--border)',
                background: provider.enabled ? 'var(--accent)' : 'var(--surface-hover)',
                opacity: busy ? 0.5 : 1,
              }}
            >
              <span
                className="absolute top-0.5 h-3 w-3 transition-all"
                style={{
                  borderRadius: '50%',
                  left: provider.enabled ? 18 : 3,
                  background: provider.enabled ? '#fff' : 'var(--text-muted)',
                }}
              />
            </span>
          </label>
        </span>
      </header>

      {/* 身：kind + base + masked key */}
      <div className="flex-1 space-y-1.5 px-4 py-3">
        <div style={{ fontSize: 'var(--text-sm)', color: 'var(--text)' }}>{provider.kind}</div>
        <div
          className="truncate"
          style={{ ...mono, fontSize: 'var(--text-xs)', color: 'var(--text-muted)' }}
          title={provider.base}
        >
          {provider.base}
        </div>

        {/* key 行 — 只出掩码，无 Show 明文 */}
        <div className="mt-2 flex items-center justify-between gap-2">
          <span
            className="flex min-w-0 items-baseline gap-2"
            style={{ fontSize: 'var(--text-sm)' }}
            title={t('card.keyTip')}
          >
            <span style={{ color: 'var(--text-muted)' }}>{t('card.keyLabel')}</span>
            <span className="tabular truncate" style={{ ...mono, color: 'var(--text)' }}>
              {provider.api_key_set
                ? (provider.api_key_masked ?? '••••••••')
                : t('card.keyUnset')}
            </span>
          </span>
          <button
            type="button"
            onClick={() => {
              setShowKeyForm((v) => !v);
              setConfirmDelete(false);
            }}
            disabled={busy}
            className="shrink-0 disabled:opacity-50"
            style={btn}
          >
            {showKeyForm ? t('common.close') : t('card.replaceKey')}
          </button>
        </div>
      </div>

      {/* 脚：最近探测结果 ∥ 火花线 — aria-live */}
      <div
        className="tabular grid grid-cols-[1fr_auto] items-center gap-3 px-4 py-2.5"
        style={{ ...rowBorder, fontSize: 'var(--text-sm)' }}
        aria-live="polite"
      >
        {probe === null ? (
          <span style={{ color: 'var(--text-muted)' }}>{t('card.lastUntested')}</span>
        ) : probe.ok ? (
          <span style={{ color: 'var(--success)' }}>
            {t('card.lastProbeOk', { ms: probe.latency_ms, status: probe.status ?? 200 })}
          </span>
        ) : (
          <span className="truncate" style={{ color: 'var(--danger)' }} title={probe.error ?? ''}>
            {t('card.lastProbeFail', {
              detail: probe.error ?? (probe.status !== null ? String(probe.status) : '—'),
            })}
          </span>
        )}
        <Sparkline hist={hist} />
      </div>

      {/* Replace key 内联表单（仅密文输入） */}
      {showKeyForm && (
        <KeyForm
          onSave={(key) => onReplaceKey(provider, key)}
          onCancel={() => setShowKeyForm(false)}
        />
      )}

      {/* 删除：普通二次确认（不再要求打字 id — v2 §3.2「过度防御」） */}
      <div className="px-4 py-2.5" style={rowBorder}>
        {!confirmDelete ? (
          <button
            type="button"
            onClick={() => {
              setConfirmDelete(true);
              setShowKeyForm(false);
            }}
            disabled={busy}
            className="disabled:opacity-50"
            style={{
              ...btn,
              background: 'transparent',
              border: '1px solid transparent',
              color: 'var(--text-muted)',
              padding: '0.35rem 0',
            }}
          >
            {t('common.delete')}
          </button>
        ) : (
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              disabled={busy}
              autoFocus
              onClick={() => {
                onDelete(provider);
                setConfirmDelete(false);
              }}
              className="disabled:cursor-not-allowed disabled:opacity-40"
              style={{
                ...btn,
                background: 'var(--danger)',
                borderColor: 'var(--danger)',
                color: '#fff',
                fontWeight: 600,
              }}
            >
              {t('card.confirmDelete')}
            </button>
            <button type="button" onClick={() => setConfirmDelete(false)} style={btn}>
              {t('common.cancel')}
            </button>
          </div>
        )}
      </div>
    </section>
  );
}
