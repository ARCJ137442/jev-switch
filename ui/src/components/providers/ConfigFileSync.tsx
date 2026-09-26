import { useCallback, useEffect, useState } from 'react';
import { exportConfigFile, getConfigFileStatus, importConfigFile, type ConfigFileStatus } from '../../api/configFile';
import { useI18n } from '../../i18n';

export function ConfigFileSync({ disabled, onImported }: { disabled: boolean; onImported: () => void }) {
  const { t } = useI18n();
  const [status, setStatus] = useState<ConfigFileStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [action, setAction] = useState<'import' | 'export' | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const refresh = useCallback(async () => {
    try { setStatus(await getConfigFileStatus()); setError(null); }
    catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); }
  }, []);
  useEffect(() => {
    let alive = true;
    const poll = async () => {
      try { const next = await getConfigFileStatus(); if (alive) { setStatus(next); setError(null); } }
      catch (cause) { if (alive) setError(cause instanceof Error ? cause.message : String(cause)); }
    };
    void poll();
    const timer = window.setInterval(poll, 15000);
    return () => { alive = false; window.clearInterval(timer); };
  }, []);
  const apply = async () => {
    if (!action || disabled || busy) return;
    setBusy(true); setError(null); setMessage(null);
    try {
      if (action === 'import') { await importConfigFile(); onImported(); }
      else await exportConfigFile();
      setMessage(t(action === 'import' ? 'configFile.imported' : 'configFile.exported'));
      setAction(null); await refresh();
    } catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); }
    finally { setBusy(false); }
  };
  return <details className="mt-6 rounded border p-4" style={{ borderColor: 'var(--border)', background: 'var(--surface)' }}>
    <summary className="cursor-pointer font-semibold">{t('configFile.title')}{status?.toml_drifted && <span className="ml-2 text-sm" style={{ color: 'var(--warning)' }}>{t('configFile.changed')}</span>}</summary>
    <p className="mt-3 text-sm" style={{ color: 'var(--text-muted)' }}>{t('configFile.summary')} {t('configFile.authority')}</p>
    {status && <><code className="mt-3 block break-all text-xs">{status.toml_path}</code><p className="mt-2 text-sm" style={{ color: status.toml_drifted ? 'var(--warning)' : 'var(--text-muted)' }}>{t(status.toml_drifted ? 'configFile.changed' : 'configFile.unchanged')}</p></>}
    {error && <p role="alert" className="mt-3 break-words text-sm" style={{ color: 'var(--danger)' }}>{error}</p>}
    {message && <p role="status" className="mt-3 text-sm" style={{ color: 'var(--success)' }}>{message}</p>}
    <div className="mt-3 flex flex-wrap gap-2">
      <button className="entry-button" disabled={disabled || busy || !status} onClick={() => { setAction('import'); setMessage(null); }}>{t('configFile.import')}</button>
      <button className="entry-button" disabled={disabled || busy || !status} onClick={() => { setAction('export'); setMessage(null); }}>{t('configFile.export')}</button>
      <button className="entry-button" disabled={busy} onClick={() => void refresh()}>{t('common.refresh')}</button>
    </div>
    {action && <div className="mt-3 rounded border p-3" style={{ borderColor: 'var(--border)' }}>
      <p className="text-sm">{t(action === 'import' ? 'configFile.importDetail' : 'configFile.exportDetail')}</p>
      <div className="mt-3 flex flex-wrap gap-2"><button className="entry-button primary" disabled={disabled || busy} onClick={() => void apply()}>{t(busy ? 'common.saving' : action === 'import' ? 'configFile.import' : 'configFile.export')}</button><button className="entry-button" disabled={busy} onClick={() => setAction(null)}>{t('common.cancel')}</button></div>
    </div>}
  </details>;
}
