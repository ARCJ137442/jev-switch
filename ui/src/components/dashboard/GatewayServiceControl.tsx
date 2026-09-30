import { useEffect, useRef, useState } from 'react';
import { LoaderCircle, Power } from 'lucide-react';
import { isTauri } from '@tauri-apps/api/core';
import { getGatewayServiceStatus, setGatewayServiceRunning } from '../../api/gatewayControl';
import { useI18n, type MessageKey } from '../../i18n';

interface Props {
  onStateChange: (running: boolean | null) => void;
}

export function GatewayServiceControl({ onStateChange }: Props) {
  const { t } = useI18n();
  const tauri = isTauri();
  const [running, setRunning] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const onStateChangeRef = useRef(onStateChange);
  useEffect(() => { onStateChangeRef.current = onStateChange; }, [onStateChange]);

  useEffect(() => {
    if (!tauri) return;
    let live = true;
    const refresh = async () => {
      try {
        const status = await getGatewayServiceStatus();
        if (live && status) {
          setRunning(status.running);
          onStateChangeRef.current(status.running);
          setError(null);
        }
      } catch (cause) {
        if (live) setError(cause instanceof Error ? cause.message : String(cause));
      }
    };
    void refresh();
    const timer = window.setInterval(() => void refresh(), 2000);
    return () => { live = false; window.clearInterval(timer); };
  }, [tauri]);

  if (!tauri) return null;
  const toggle = async () => {
    if (busy || running === null) return;
    const next = !running;
    setBusy(true);
    setError(null);
    try {
      const status = await setGatewayServiceRunning(next);
      setRunning(status.running);
      onStateChangeRef.current(status.running);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  };
  const label = running === null ? t('gateway.unavailable' as MessageKey) : t(running ? 'gateway.running' as MessageKey : 'gateway.stopped' as MessageKey);

  return <div className="gateway-service-control">
    <button type="button" role="switch" aria-checked={running === true} aria-label={label} title={label} disabled={busy || running === null} onClick={() => void toggle()} className="gateway-service-control__switch" data-running={running === true}>
      {busy ? <LoaderCircle size={15} className="animate-spin" aria-hidden="true" /> : <Power size={15} aria-hidden="true" />}
      <span>{t(running ? 'gateway.stop' as MessageKey : 'gateway.start' as MessageKey)}</span>
    </button>
    {error && <span className="gateway-service-control__error" role="status" title={error}>{t('gateway.controlError' as MessageKey)}</span>}
  </div>;
}
