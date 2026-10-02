import { useEffect, useState } from 'react';
import { Activity, Download, FileText, RefreshCw } from 'lucide-react';
import { getBase } from '../../api';
import { exportAndroidDebugLog, getAndroidDebugLogStatus, readAndroidDebugLog, recordAndroidWebProbe, setAndroidDebugLog, type AndroidDebugLogStatus } from '../../api/androidDebug';
import { probeAndroidGateway, type AndroidGatewayProbe } from '../../api/androidGateway';
import { useI18n, type MessageKey } from '../../i18n';

interface WebProbe {
  url: string;
  status: number | null;
  identityOk: boolean;
  error: string | null;
}

async function probeWebHealth(): Promise<WebProbe> {
  const url = `${getBase() || window.location.origin}/health`;
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), 3000);
  try {
    const response = await fetch(url, { signal: controller.signal });
    const body: unknown = await response.json().catch(() => null);
    const identityOk = response.ok && typeof body === 'object' && body !== null
      && (body as Record<string, unknown>).product === 'jev-switch'
      && (body as Record<string, unknown>).status === 'ok';
    return { url, status: response.status, identityOk, error: identityOk ? null : 'Unexpected /health response' };
  } catch (error) {
    return { url, status: null, identityOk: false, error: error instanceof Error ? error.message : String(error) };
  } finally {
    window.clearTimeout(timeout);
  }
}

const buttonClass = 'inline-flex min-h-10 items-center gap-2 rounded border px-3 text-sm';
const buttonStyle = { borderColor: 'var(--border)', color: 'var(--text)', background: 'var(--surface)' };

export function AndroidDiagnostics() {
  const { t } = useI18n();
  const [logStatus, setLogStatus] = useState<AndroidDebugLogStatus | null>(null);
  const [logText, setLogText] = useState<string | null>(null);
  const [nativeProbe, setNativeProbe] = useState<AndroidGatewayProbe | null>(null);
  const [webProbe, setWebProbe] = useState<WebProbe | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void getAndroidDebugLogStatus().then((status) => { if (active) setLogStatus(status); }).catch((cause) => {
      if (active) setError(String(cause));
    });
    return () => { active = false; };
  }, []);

  const diagnose = async () => {
    setBusy(true);
    setError(null);
    const [native, web] = await Promise.allSettled([probeAndroidGateway(), probeWebHealth()]);
    setNativeProbe(native.status === 'fulfilled' ? native.value : {
      running: false, bind: null, http_status: null, identity_ok: false, error: String(native.reason),
    });
    const webResult = web.status === 'fulfilled' ? web.value : {
      url: `${getBase() || window.location.origin}/health`, status: null, identityOk: false, error: String(web.reason),
    };
    setWebProbe(webResult);
    void recordAndroidWebProbe(webResult.identityOk, webResult.status).catch(() => undefined);
    setBusy(false);
  };

  const viewLog = async () => {
    setError(null);
    try { setLogText(await readAndroidDebugLog()); }
    catch (cause) { setError(String(cause)); }
  };

  const exportLog = async () => {
    setError(null);
    setMessage(null);
    try {
      const result = await exportAndroidDebugLog();
      setMessage(t(result === 'downloads' ? 'settings.androidDebugSaved' as MessageKey : 'settings.androidDebugShared' as MessageKey));
    } catch (cause) { setError(String(cause)); }
  };

  return <div className="grid gap-3 text-sm">
    <label className="flex items-start gap-3">
      <input type="checkbox" checked={logStatus?.enabled === true} onChange={async (event) => {
        setError(null);
        try { setLogStatus(await setAndroidDebugLog(event.target.checked)); }
        catch (cause) { setError(String(cause)); }
      }} className="mt-1 h-4 w-4 accent-[var(--accent)]" />
      <span className="grid gap-1"><strong>{t('settings.androidDebugToggle' as MessageKey)}</strong><span style={{ color: 'var(--text-muted)' }}>{t('settings.androidDebugHint' as MessageKey)}</span></span>
    </label>
    <div className="flex flex-wrap gap-2">
      <button type="button" className={buttonClass} style={buttonStyle} onClick={() => void diagnose()} disabled={busy}><Activity size={15} aria-hidden="true" />{t('settings.androidDiagnose' as MessageKey)}</button>
      <button type="button" className={buttonClass} style={buttonStyle} onClick={() => void viewLog()}><FileText size={15} aria-hidden="true" />{t('settings.androidViewLog' as MessageKey)}</button>
      <button type="button" className={buttonClass} style={buttonStyle} onClick={() => void exportLog()}><Download size={15} aria-hidden="true" />{t('settings.androidExportLog' as MessageKey)}</button>
    </div>
    {(nativeProbe || webProbe) && <div className="grid gap-2 rounded border p-3 text-xs" style={{ borderColor: 'var(--border)' }} role="status">
      <strong>{t('settings.androidDiagnosticResult' as MessageKey)}</strong>
      {nativeProbe && <p className="break-all" style={{ color: nativeProbe.identity_ok ? 'var(--success)' : 'var(--danger)' }}>{t('settings.androidNativeProbe' as MessageKey)}: {nativeProbe.running ? nativeProbe.bind : t('gateway.stopped' as MessageKey)} · HTTP {nativeProbe.http_status ?? '—'} · {nativeProbe.identity_ok ? 'OK' : nativeProbe.error ?? 'unavailable'}</p>}
      {webProbe && <p className="break-all" style={{ color: webProbe.identityOk ? 'var(--success)' : 'var(--danger)' }}>{t('settings.androidWebProbe' as MessageKey)}: {webProbe.url} · HTTP {webProbe.status ?? '—'} · {webProbe.identityOk ? 'OK' : webProbe.error ?? 'unavailable'}</p>}
      <p style={{ color: 'var(--text-muted)' }}>{t('settings.androidProbeHint' as MessageKey, { origin: window.location.origin })}</p>
    </div>}
    {logText !== null && <div className="grid gap-2"><strong>{t('settings.androidLogContents' as MessageKey)}</strong><pre className="whitespace-pre-wrap break-all rounded border p-3 text-xs select-text" style={{ borderColor: 'var(--border)', background: 'var(--surface)' }}>{logText || t('settings.androidLogEmpty' as MessageKey)}</pre><button type="button" className={buttonClass + ' w-fit'} style={buttonStyle} onClick={() => void viewLog()}><RefreshCw size={15} aria-hidden="true" />{t('settings.androidRefreshLog' as MessageKey)}</button></div>}
    {message && <p role="status" style={{ color: 'var(--success)' }}>{message}</p>}
    {error && <p role="alert" className="break-all" style={{ color: 'var(--danger)' }}>{error}</p>}
  </div>;
}
