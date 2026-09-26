import { useState } from 'react';
import type { AdminProvider, ProbeResponse } from '../../api/admin';
import { KeyForm } from './KeyForm';
import { ProviderConfigForm, type ProviderConfigFields } from './ProviderConfigForm';
import { ProbeButton } from './ProbeButton';
import { useI18n, type MessageKey } from '../../i18n';

interface Props {
  provider: AdminProvider;
  /** 该卡有 PUT 在途（toggle/key/delete） */
  busy: boolean;
  onToggle: (provider: AdminProvider, enabled: boolean) => void;
  onReplaceKey: (provider: AdminProvider, apiKey: string) => Promise<void>;
  onClearKey: (provider: AdminProvider) => void;
  onUpdate: (provider: AdminProvider, fields: ProviderConfigFields) => Promise<void>;
  onDelete: (provider: AdminProvider) => void;
}

type Connectivity = 'reachable' | 'unreachable' | 'off' | 'unknown';

/** Probe measures endpoint connectivity only, never model inference health. */
function connectionOf(provider: AdminProvider, probe: ProbeResponse | null): Connectivity {
  if (!provider.enabled) return 'off';
  if (probe === null) return 'unknown';
  return probe.ok ? 'reachable' : 'unreachable';
}

const CONNECTION_COLOR: Record<Connectivity, string> = {
  reachable: 'var(--success)',
  unreachable: 'var(--danger)',
  off: 'var(--text-subtle)',
  unknown: 'var(--text-subtle)',
};

const CONNECTION_PULSE: Record<Connectivity, string> = {
  reachable: '',
  unreachable: 'status-danger',
  off: '',
  unknown: '',
};

const CONNECTION_LABEL: Record<Connectivity, MessageKey> = {
  reachable: 'providers.probeReachable',
  unreachable: 'providers.probeUnreachable',
  off: 'card.disabled',
  unknown: 'providers.probeUntested',
};

/** Endpoint probe latency history; it does not represent inference health. */
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
 * 一个接入配置一张卡：账号、地址、密钥、适配器 kind 与模型资源共同归属此卡。
 * 密钥只出掩码（契约 04 §2），卡片内不存在任何明文；说明文字全部走 title tooltip。
 */
export function ProviderCard({ provider, busy, onToggle, onReplaceKey, onClearKey, onUpdate, onDelete }: Props) {
  const [probe, setProbe] = useState<ProbeResponse | null>(null);
  const [hist, setHist] = useState<ReadonlyArray<{ ms: number; ok: boolean }>>([]);
  const [showKeyForm, setShowKeyForm] = useState(false);
  const [showConfigForm, setShowConfigForm] = useState(false);
  const [confirmClearKey, setConfirmClearKey] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const onProbe = (r: ProbeResponse) => {
    setProbe(r);
    setHist((h) => [...h.slice(-23), { ms: r.latency_ms, ok: r.ok }]);
  };

  const { t } = useI18n();
  const connectivity = connectionOf(provider, probe);
  const label = t(CONNECTION_LABEL[connectivity]);

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
      aria-label={`${t('providers.accountCard')} ${provider.id}`}
    >
      {/* 头：● Healthy · id — Probe + 开关 */}
      <header
        className="flex items-center justify-between gap-3 px-4 py-3"
        style={{ borderBottom: '1px solid var(--border)' }}
      >
        <span className="flex min-w-0 items-center gap-2.5" aria-live="polite">
          <span
            className={CONNECTION_PULSE[connectivity]}
            style={{
              width: 10,
              height: 10,
              borderRadius: '50%',
              background: CONNECTION_COLOR[connectivity],
              display: 'inline-block',
              flexShrink: 0,
            }}
            aria-hidden
          />
          <span
            className="font-semibold"
            style={{ fontSize: 'var(--text-base)', color: CONNECTION_COLOR[connectivity] }}
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
              aria-label={t((provider.enabled ? 'providers.disableAccount' : 'providers.enableAccount') as MessageKey, { id: provider.id })}
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

      {/* One composite provider account config: endpoint, account identity, credentials and model resources. */}
      <div className="flex-1 space-y-1.5 px-4 py-3">
        <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
          <span className="font-semibold" style={{ fontSize: 'var(--text-sm)', color: 'var(--text)' }}>{provider.name || provider.account || provider.id}</span>
          {provider.account && <span style={{ fontSize: 'var(--text-xs)', color: 'var(--text-muted)' }}>{t('providers.account' as MessageKey)} · {provider.account}</span>}
        </div>
        <div style={{ fontSize: 'var(--text-xs)', color: 'var(--text-muted)' }}>{provider.kind}</div>
        <div
          className="truncate"
          style={{ ...mono, fontSize: 'var(--text-xs)', color: 'var(--text-muted)' }}
          title={provider.base}
        >
          {provider.base}
        </div>

        <div className="mt-3">
          <div className="mb-1 flex items-baseline justify-between gap-2">
            <span style={{ fontSize: 'var(--text-xs)', color: 'var(--text-muted)' }}>{t('providers.models' as MessageKey)}</span>
            <span className="tabular" style={{ fontSize: 'var(--text-xs)', color: 'var(--text-subtle)' }}>{t('providers.modelsCount' as MessageKey, { n: provider.models?.length ?? 0 })}</span>
          </div>
          {provider.models && provider.models.length > 0 ? (
            <div className="flex flex-wrap gap-1.5">
              {provider.models.map((model) => <code key={model} className="max-w-full break-all border px-1.5 py-0.5" style={{ borderColor: 'var(--border)', borderRadius: 'var(--radius)', background: 'var(--surface-hover)', color: 'var(--text)', fontSize: 'var(--text-xs)' }}>{model}</code>)}
            </div>
          ) : <span style={{ fontSize: 'var(--text-xs)', color: 'var(--text-subtle)' }}>{t('providers.noModels' as MessageKey)}</span>}
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
          <span className="flex shrink-0 gap-1.5">
          <button
            type="button"
            onClick={() => {
              setShowKeyForm((v) => !v);
              setShowConfigForm(false);
              setConfirmClearKey(false);
              setConfirmDelete(false);
            }}
            disabled={busy}
            className="shrink-0 disabled:opacity-50"
            style={btn}
          >
            {showKeyForm ? t('common.close') : t('card.replaceKey')}
          </button>
          {provider.api_key_set && <button type="button" onClick={() => { setConfirmClearKey((value) => !value); setShowKeyForm(false); setShowConfigForm(false); }} disabled={busy} style={btn}>{t('providers.clearKey' as MessageKey)}</button>}
          </span>
        </div>
        {confirmClearKey && <div className="mt-2 flex flex-wrap items-center gap-2 text-xs">
          <span style={{ color: 'var(--warning)' }}>{t('providers.confirmClearKey' as MessageKey)}</span>
          <button type="button" disabled={busy} onClick={() => { onClearKey(provider); setConfirmClearKey(false); }} style={{ ...btn, color: 'var(--danger)' }}>{t('providers.clearKey' as MessageKey)}</button>
          <button type="button" onClick={() => setConfirmClearKey(false)} style={btn}>{t('common.cancel')}</button>
        </div>}
        <div className="mt-3">
          <button type="button" disabled={busy} onClick={() => { setShowConfigForm((value) => !value); setShowKeyForm(false); setConfirmClearKey(false); setConfirmDelete(false); }} style={btn}>{showConfigForm ? t('common.close') : t('providers.editConfig' as MessageKey)}</button>
        </div>
      </div>

      {/* 脚：最近探测结果 ∥ 火花线 — aria-live */}
      <div
        className="tabular grid grid-cols-[1fr_auto] items-center gap-3 px-4 py-2.5"
        style={{ ...rowBorder, fontSize: 'var(--text-sm)' }}
        aria-live="polite"
      >
        {probe === null ? (
          <span style={{ color: 'var(--text-muted)' }}>{t('providers.probeUntested' as MessageKey)}</span>
        ) : probe.ok ? (
          <span style={{ color: 'var(--success)' }}>
            {t('card.lastProbeOk', { ms: probe.latency_ms, status: probe.status > 0 ? probe.status : '—' })}
          </span>
        ) : (
          <span className="truncate" style={{ color: 'var(--danger)' }} title={probe.error ?? ''}>
            {t('card.lastProbeFail', {
              detail: probe.error ?? (probe.status > 0 ? String(probe.status) : '—'),
            })}
          </span>
        )}
        <Sparkline hist={hist} />
      </div>

      <div className="px-4 pb-2 text-xs" style={{ color: 'var(--text-subtle)' }}>{t('providers.probeOnly' as MessageKey)}</div>

      {showConfigForm && <ProviderConfigForm provider={provider} busy={busy} onSave={(fields) => onUpdate(provider, fields)} onCancel={() => setShowConfigForm(false)} />}

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
