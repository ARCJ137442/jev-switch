import { useEffect, useRef, useState } from 'react';
import { LoaderCircle, Power } from 'lucide-react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getGatewayServiceStatus, setGatewayServiceRunning } from '../../api/gatewayControl';
import { getAndroidNotificationPermissionState, isAndroidTauri, requestAndroidNotificationPermission } from '../../api/androidGateway';
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
  const quickToggleRef = useRef<() => Promise<void>>(async () => undefined);
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
  const toggle = async (fromQuickSettings = false) => {
    if (busy || (!fromQuickSettings && running === null)) return;
    setBusy(true);
    setError(null);
    try {
      let currentRunning = running;
      if (fromQuickSettings) {
        const status = await getGatewayServiceStatus();
        currentRunning = status?.running ?? null;
      }
      if (currentRunning === null) throw new Error('Gateway state is unavailable.');
      if (isAndroidTauri() && !currentRunning) {
        const permission = await getAndroidNotificationPermissionState();
        if (permission !== 'granted') await requestAndroidNotificationPermission();
      }
      const next = !currentRunning;
      const status = await setGatewayServiceRunning(next);
      setRunning(status.running);
      onStateChangeRef.current(status.running);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  };
  quickToggleRef.current = async () => toggle(true);
  useEffect(() => {
    if (!isAndroidTauri()) return;
    let active = true;
    let unlisten: (() => void) | null = null;
    const consume = async () => {
      try {
        const result = await invoke<{ pending: boolean }>('plugin:jev-android-keepalive|takePendingGatewayToggle');
        if (active && result.pending) await quickToggleRef.current();
      } catch {
        // The notification and Dashboard controls remain available if tile startup fails.
      }
    };
    void consume();
    void listen('jev-switch-gateway-toggle', () => void consume()).then((stop) => {
      if (active) unlisten = stop;
      else stop();
    });
    return () => { active = false; unlisten?.(); };
  }, []);

  if (!tauri) return null;
  const label = running === null ? t('gateway.unavailable' as MessageKey) : t(running ? 'gateway.running' as MessageKey : 'gateway.stopped' as MessageKey);

  return <div className="gateway-service-control">
    <button type="button" role="switch" aria-checked={running === true} aria-label={label} title={label} disabled={busy || running === null} onClick={() => void toggle()} className="gateway-service-control__switch" data-running={running === true}>
      {busy ? <LoaderCircle size={15} className="animate-spin" aria-hidden="true" /> : <Power size={15} aria-hidden="true" />}
      <span>{t(running ? 'gateway.stop' as MessageKey : 'gateway.start' as MessageKey)}</span>
    </button>
    {error && <span className="gateway-service-control__error" role="status" title={error}>{t('gateway.controlError' as MessageKey)}</span>}
  </div>;
}
