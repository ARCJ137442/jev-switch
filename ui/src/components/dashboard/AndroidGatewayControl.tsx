import { useEffect, useState } from 'react';
import { CircleStop, Play, RefreshCw, Smartphone } from 'lucide-react';
import { getAndroidGatewayStatus, isAndroidTauri, startAndroidGateway, stopAndroidGateway, type AndroidGatewayStatus } from '../../api/androidGateway';
import { useI18n, type MessageKey } from '../../i18n';

export function AndroidGatewayControl() {
  const { t } = useI18n();
  const [status, setStatus] = useState<AndroidGatewayStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isAndroidTauri()) return;
    let cancelled = false;
    const refresh = async () => {
      try {
        const next = await getAndroidGatewayStatus();
        if (!cancelled) { setStatus(next); setError(null); }
      } catch (cause) {
        if (!cancelled) setError(cause instanceof Error ? cause.message : String(cause));
      }
    };
    void refresh();
    const timer = window.setInterval(() => void refresh(), 2000);
    return () => { cancelled = true; window.clearInterval(timer); };
  }, []);

  if (!isAndroidTauri()) return null;

  const run = async (action: () => Promise<AndroidGatewayStatus>) => {
    setBusy(true);
    setError(null);
    try { setStatus(await action()); }
    catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); }
    finally { setBusy(false); }
  };

  return <section className="fade-in mb-5 p-4 sm:p-5" style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 'var(--radius)' }}>
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div className="flex min-w-0 items-center gap-2.5">
        <span className="inline-grid h-8 w-8 shrink-0 place-items-center" style={{ color: 'var(--accent)', background: 'color-mix(in srgb, var(--accent) 12%, transparent)', borderRadius: 'var(--radius)' }}><Smartphone size={17} aria-hidden="true" /></span>
        <div className="min-w-0"><h2 className="font-semibold" style={{ fontSize: 'var(--text-lg)' }}>{t('android.gatewayTitle' as MessageKey)}</h2><p className="text-xs" style={{ color: 'var(--text-muted)' }}>{status?.running ? `${t('android.running' as MessageKey)} · ${status.bind}` : t('android.stopped' as MessageKey)}</p></div>
      </div>
      <div className="flex flex-wrap gap-2">
        <button type="button" disabled={busy || status?.running === true} onClick={() => void run(startAndroidGateway)} className="inline-flex min-h-9 items-center gap-2 border px-3 text-sm font-semibold" style={{ borderColor: 'var(--accent)', borderRadius: 'var(--radius)', background: status?.running ? 'var(--surface-hover)' : 'var(--accent)', color: status?.running ? 'var(--text-muted)' : '#fff' }}><Play size={14} aria-hidden="true" />{t('android.start' as MessageKey)}</button>
        <button type="button" disabled={busy || status?.running !== true} onClick={() => void run(stopAndroidGateway)} className="inline-flex min-h-9 items-center gap-2 border px-3 text-sm" style={{ borderColor: 'var(--border)', borderRadius: 'var(--radius)', background: 'var(--surface)', color: 'var(--text)' }}><CircleStop size={14} aria-hidden="true" />{t('android.stop' as MessageKey)}</button>
        <button type="button" disabled={busy} onClick={() => void run(async () => (await getAndroidGatewayStatus()) ?? { running: false, bind: null, desired_running: false })} className="inline-grid h-9 w-9 place-items-center border" style={{ borderColor: 'var(--border)', borderRadius: 'var(--radius)', color: 'var(--text-muted)' }} title={t('common.refresh')} aria-label={t('common.refresh')}><RefreshCw size={14} aria-hidden="true" /></button>
      </div>
    </div>
    {error && <p role="alert" className="mt-3 text-xs" style={{ color: 'var(--danger)' }}>{error}</p>}
  </section>;
}
