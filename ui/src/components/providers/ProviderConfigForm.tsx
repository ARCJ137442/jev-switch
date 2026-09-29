import { useState } from 'react';
import { discoverProviderModels, type AdminProvider } from '../../api/admin';
import { applyProviderKindPreset, isProviderKind } from '../../api/providerKinds';
import { useI18n, type MessageKey } from '../../i18n';
import { ProviderKindSelect } from './ProviderKindSelect';

export interface ProviderConfigFields {
  name: string | null;
  account: string | null;
  kind: string;
  base: string;
  models: string[];
}

interface Props {
  provider: AdminProvider;
  busy: boolean;
  onSave: (fields: ProviderConfigFields) => Promise<void>;
  onCancel: () => void;
}

const inputStyle: React.CSSProperties = {
  width: '100%',
  minHeight: 34,
  padding: '6px 9px',
  border: '1px solid var(--border)',
  borderRadius: 'var(--radius)',
  background: 'var(--surface)',
  color: 'var(--text)',
  fontFamily: 'var(--font-mono)',
  fontSize: 'var(--text-sm)',
};

export function ProviderConfigForm({ provider, busy, onSave, onCancel }: Props) {
  const { t } = useI18n();
  const [name, setName] = useState(provider.name ?? '');
  const [account, setAccount] = useState(provider.account ?? '');
  const [kind, setKind] = useState(provider.kind);
  const [base, setBase] = useState(provider.base);
  const [models, setModels] = useState((provider.models ?? []).join('\n'));
  const [error, setError] = useState<string | null>(null);
  const [discovering, setDiscovering] = useState(false);
  const [discoverMessage, setDiscoverMessage] = useState<string | null>(null);

  const selectKind = (nextKind: string) => {
    const currentModels = models.split(/\r?\n/).map((model) => model.trim()).filter(Boolean);
    const preset = applyProviderKindPreset(kind, nextKind, base, currentModels);
    setKind(nextKind);
    setBase(preset.base);
    setModels(preset.models.join('\n'));
  };

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!kind.trim() || !isProviderKind(kind.trim()) || !/^https?:\/\/.+/.test(base.trim())) {
      setError(!kind.trim() ? t('add.kindRequired') : !isProviderKind(kind.trim()) ? t('add.kindUnsupported', { kind: kind.trim() }) : t('add.baseHttp'));
      return;
    }
    setError(null);
    try {
      await onSave({
        name: name.trim() || null,
        account: account.trim() || null,
        kind: kind.trim(),
        base: base.trim(),
        models: [...new Set(models.split(/\r?\n/).map((model) => model.trim()).filter(Boolean))],
      });
      onCancel();
    } catch (cause) {
      setError((cause as Error).message);
    }
  };

  const discoverModels = async () => {
    setDiscovering(true);
    setDiscoverMessage(null);
    try {
      const result = await discoverProviderModels(provider.id);
      if (!result.ok || result.models.length === 0) {
        setDiscoverMessage(t('providers.modelsDiscoverFailed' as MessageKey, { error: result.error ?? t('providers.noModels' as MessageKey) }));
        return;
      }
      setModels(result.models.join('\n'));
      setDiscoverMessage(t('providers.modelsDiscovered' as MessageKey, { n: result.models.length, status: result.status, ms: result.latency_ms }));
    } catch (cause) {
      setDiscoverMessage(t('providers.modelsDiscoverFailed' as MessageKey, { error: (cause as Error).message }));
    } finally {
      setDiscovering(false);
    }
  };

  const labelStyle: React.CSSProperties = { color: 'var(--text-muted)', fontSize: 'var(--text-xs)' };
  const button: React.CSSProperties = { minHeight: 32, padding: '0 10px', border: '1px solid var(--border)', borderRadius: 'var(--radius)', background: 'var(--surface)', color: 'var(--text)', fontSize: 'var(--text-sm)' };

  return (
    <form onSubmit={(event) => void submit(event)} className="space-y-3 px-4 py-4" style={{ borderTop: '1px solid var(--border)', background: 'var(--surface-hover)' }}>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        <label className="flex flex-col gap-1.5">
          <span style={labelStyle}>{t('providers.id' as MessageKey)}</span>
          <input value={provider.id} readOnly aria-readonly="true" style={{ ...inputStyle, opacity: 0.7 }} />
          <span style={{ ...labelStyle, fontSize: 'var(--text-xs)' }}>{t('providers.idLocked' as MessageKey)}</span>
        </label>
        <label className="flex flex-col gap-1.5">
          <span style={labelStyle}>{t('providers.name' as MessageKey)}</span>
          <input value={name} onChange={(event) => setName(event.target.value)} style={{ ...inputStyle, fontFamily: 'var(--font-sans)' }} />
        </label>
        <label className="flex flex-col gap-1.5">
          <span style={labelStyle}>{t('providers.account' as MessageKey)}</span>
          <input value={account} onChange={(event) => setAccount(event.target.value)} style={{ ...inputStyle, fontFamily: 'var(--font-sans)' }} />
        </label>
        <label className="flex flex-col gap-1.5">
          <span style={labelStyle}>{t('providers.kind' as MessageKey)}</span>
          <ProviderKindSelect value={kind} onChange={selectKind} style={inputStyle} />
        </label>
        <label className="flex flex-col gap-1.5 sm:col-span-2">
          <span style={labelStyle}>{t('providers.base' as MessageKey)}</span>
          <input value={base} onChange={(event) => setBase(event.target.value)} style={inputStyle} />
        </label>
      </div>
      <label className="flex flex-col gap-1.5">
        <span className="flex items-center justify-between gap-2" style={labelStyle}>
          <span>{t('providers.models' as MessageKey)}</span>
          <button type="button" disabled={busy || discovering} onClick={() => void discoverModels()} style={{ ...button, minHeight: 28, padding: '0 8px', fontSize: 'var(--text-xs)' }}>
            {discovering ? t('providers.discoveringModels' as MessageKey) : t('providers.discoverModels' as MessageKey)}
          </button>
        </span>
        <textarea value={models} onChange={(event) => setModels(event.target.value)} rows={4} spellCheck={false} className="resize-y p-2.5 leading-relaxed" style={inputStyle} />
        <span style={labelStyle}>{t('providers.modelsHint' as MessageKey)}</span>
        {discoverMessage && <span role="status" style={{ ...labelStyle, color: 'var(--text-muted)' }}>{discoverMessage}</span>}
      </label>
      {error && <p role="alert" className="text-sm" style={{ color: 'var(--danger)' }}>{error}</p>}
      <div className="flex gap-2">
        <button type="submit" disabled={busy} className="disabled:opacity-50" style={{ ...button, borderColor: 'var(--accent)', background: 'var(--accent)', color: '#fff', fontWeight: 600 }}>{t('providers.saveConfig' as MessageKey)}</button>
        <button type="button" disabled={busy} onClick={onCancel} style={button}>{t('common.cancel')}</button>
      </div>
    </form>
  );
}
