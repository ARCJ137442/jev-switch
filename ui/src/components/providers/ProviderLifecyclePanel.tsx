import { useEffect, useMemo, useState } from 'react';
import { Play, RefreshCw, Save, Square, Terminal } from 'lucide-react';
import {
  getProviderLifecycle,
  providerLifecycleStatus,
  putProviderLifecycle,
  startProviderService,
  stopProviderService,
  type AdminProvider,
} from '../../api/admin';
import type { ProviderLifecycleConfig } from '../../generated/ProviderLifecycleConfig';
import type { LifecycleStatus } from '../../generated/LifecycleStatus';
import { useI18n, type MessageKey } from '../../i18n';
import { useToast } from '../../app/feedback';

interface Props {
  provider: AdminProvider;
  busy: boolean;
}

const inputStyle: React.CSSProperties = {
  border: '1px solid var(--border)',
  borderRadius: 'var(--radius)',
  background: 'var(--surface)',
  color: 'var(--text)',
  padding: '0.4rem 0.55rem',
  fontSize: 'var(--text-sm)',
};

function defaultCommand(provider: AdminProvider): ProviderLifecycleConfig {
  return {
    controllable: provider.lifecycle.controllable,
    process_policy: provider.lifecycle.process_policy,
    args: [],
    stop_args: [],
    status_args: [],
    inject_api_key: false,
    timeout_ms: provider.lifecycle.timeout_ms,
    readiness_timeout_ms: provider.lifecycle.readiness_timeout_ms,
    mode: provider.lifecycle.mode,
  };
}

function lines(values: string): string[] {
  return values.split(/\r?\n/).map((value) => value.trim()).filter(Boolean);
}

function statusTone(status: LifecycleStatus | null): string {
  if (!status) return 'var(--text-muted)';
  if (status.state === 'running') return 'var(--success)';
  if (status.state === 'failed' || status.state === 'unknown') return 'var(--warning)';
  if (status.state === 'disabled' || status.state === 'uncontrolled') return 'var(--text-subtle)';
  return 'var(--accent)';
}

export function ProviderLifecyclePanel({ provider, busy }: Props) {
  const { t } = useI18n();
  const { toast } = useToast();
  const [command, setCommand] = useState<ProviderLifecycleConfig>(() => defaultCommand(provider));
  const [status, setStatus] = useState<LifecycleStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [action, setAction] = useState<'start' | 'stop' | 'status' | null>(null);
  const [secretWarning, setSecretWarning] = useState(false);
  const [secretCountdown, setSecretCountdown] = useState(5);

  useEffect(() => {
    let active = true;
    setLoading(true);
    void getProviderLifecycle(provider.id)
      .then((doc) => {
        if (!active) return;
        setCommand(doc.command);
        setStatus(doc.status);
      })
      .catch((error) => {
        if (active) toast('danger', t('providers.lifecycleLoadFailed' as MessageKey, { reason: (error as Error).message }));
      })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [provider.id, toast, t]);

  useEffect(() => {
    if (!secretWarning) return;
    setSecretCountdown(5);
    const timer = window.setInterval(() => setSecretCountdown((value) => Math.max(0, value - 1)), 1000);
    return () => window.clearInterval(timer);
  }, [secretWarning]);

  const commandFields = useMemo(() => ({
    args: command.args.join('\n'),
    stop_args: command.stop_args.join('\n'),
    status_args: command.status_args.join('\n'),
  }), [command.args, command.stop_args, command.status_args]);

  const update = <K extends keyof ProviderLifecycleConfig>(key: K, value: ProviderLifecycleConfig[K]) => {
    setCommand((current) => ({ ...current, [key]: value }));
  };

  const save = async () => {
    if (command.inject_api_key && secretCountdown > 0) {
      setSecretWarning(true);
      return;
    }
    setSaving(true);
    try {
      const doc = await putProviderLifecycle(provider.id, command);
      setCommand(doc.command);
      setStatus(doc.status);
      setSecretWarning(false);
      toast('ok', t('providers.lifecycleSaved' as MessageKey));
    } catch (error) {
      toast('danger', t('providers.lifecycleSaveFailed' as MessageKey, { reason: (error as Error).message }));
    } finally {
      setSaving(false);
    }
  };

  const run = async (kind: 'start' | 'stop' | 'status') => {
    setAction(kind);
    try {
      const next = kind === 'start'
        ? await startProviderService(provider.id)
        : kind === 'stop'
          ? await stopProviderService(provider.id)
          : await providerLifecycleStatus(provider.id);
      setStatus(next);
      toast(next.state === 'running' ? 'ok' : next.state === 'unknown' ? 'warn' : 'danger', t(`providers.lifecycle${kind[0].toUpperCase()}${kind.slice(1)}Result` as MessageKey, { state: next.state }));
    } catch (error) {
      toast('danger', t('providers.lifecycleActionFailed' as MessageKey, { reason: (error as Error).message }));
    } finally {
      setAction(null);
    }
  };

  return (
    <div className="mt-3 grid gap-3 border-t pt-3" style={{ borderColor: 'var(--border)' }}>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Terminal size={15} aria-hidden="true" style={{ color: 'var(--accent)' }} />
          <strong style={{ fontSize: 'var(--text-sm)' }}>{t('providers.lifecycleTitle' as MessageKey)}</strong>
          <span className="font-mono text-xs" style={{ color: statusTone(status) }}>{status?.state ?? (loading ? '…' : 'unknown')}</span>
        </div>
        <div className="flex flex-wrap gap-1.5">
          <button type="button" disabled={busy || action !== null} onClick={() => void run('status')} style={inputStyle} title={t('providers.lifecycleCheckHint' as MessageKey)}><RefreshCw size={14} aria-hidden="true" /> {t('providers.lifecycleCheck' as MessageKey)}</button>
          <button type="button" disabled={busy || action !== null || !command.controllable} onClick={() => void run('start')} style={{ ...inputStyle, borderColor: 'var(--success)' }}><Play size={14} aria-hidden="true" /> {t('providers.lifecycleStart' as MessageKey)}</button>
          <button type="button" disabled={busy || action !== null || !command.controllable} onClick={() => void run('stop')} style={{ ...inputStyle, borderColor: 'var(--danger)' }}><Square size={14} aria-hidden="true" /> {t('providers.lifecycleStop' as MessageKey)}</button>
        </div>
      </div>

      <div className="grid gap-3 md:grid-cols-2">
        <label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={command.controllable} onChange={(event) => update('controllable', event.target.checked)} />{t('providers.lifecycleControllable' as MessageKey)}</label>
        <label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={command.inject_api_key} onChange={(event) => update('inject_api_key', event.target.checked)} />{t('providers.lifecycleInjectKey' as MessageKey)}</label>
        <label className="grid gap-1 text-xs" style={{ color: 'var(--text-muted)' }}>{t('providers.lifecycleMode' as MessageKey)}<select value={command.mode} onChange={(event) => update('mode', event.target.value)} style={inputStyle}><option value="manual">manual</option><option value="startup_check">startup_check</option><option value="on_demand">on_demand</option></select></label>
        <label className="grid gap-1 text-xs" style={{ color: 'var(--text-muted)' }}>{t('providers.lifecyclePolicy' as MessageKey)}<select value={command.process_policy} onChange={(event) => update('process_policy', event.target.value as ProviderLifecycleConfig['process_policy'])} style={inputStyle}><option value="persistent">persistent</option><option value="session">session</option><option value="external">external</option></select></label>
        <label className="grid gap-1 text-xs md:col-span-2" style={{ color: 'var(--text-muted)' }}>{t('providers.lifecycleProgram' as MessageKey)}<input value={command.program ?? ''} onChange={(event) => update('program', event.target.value || null)} placeholder="C:\\Tools\\Laya\\laya.exe" style={{ ...inputStyle, fontFamily: 'var(--font-mono)' }} /></label>
        <label className="grid gap-1 text-xs" style={{ color: 'var(--text-muted)' }}>{t('providers.lifecycleArgs' as MessageKey)}<textarea value={commandFields.args} onChange={(event) => update('args', lines(event.target.value))} rows={3} placeholder="--port\n18765" style={{ ...inputStyle, fontFamily: 'var(--font-mono)' }} /></label>
        <label className="grid gap-1 text-xs" style={{ color: 'var(--text-muted)' }}>{t('providers.lifecycleStopProgram' as MessageKey)}<input value={command.stop_program ?? ''} onChange={(event) => update('stop_program', event.target.value || null)} style={{ ...inputStyle, fontFamily: 'var(--font-mono)' }} /></label>
        <label className="grid gap-1 text-xs" style={{ color: 'var(--text-muted)' }}>{t('providers.lifecycleStopArgs' as MessageKey)}<textarea value={commandFields.stop_args} onChange={(event) => update('stop_args', lines(event.target.value))} rows={2} style={{ ...inputStyle, fontFamily: 'var(--font-mono)' }} /></label>
        <label className="grid gap-1 text-xs" style={{ color: 'var(--text-muted)' }}>{t('providers.lifecycleStatusProgram' as MessageKey)}<input value={command.status_program ?? ''} onChange={(event) => update('status_program', event.target.value || null)} style={{ ...inputStyle, fontFamily: 'var(--font-mono)' }} /></label>
        <label className="grid gap-1 text-xs" style={{ color: 'var(--text-muted)' }}>{t('providers.lifecycleStatusArgs' as MessageKey)}<textarea value={commandFields.status_args} onChange={(event) => update('status_args', lines(event.target.value))} rows={2} style={{ ...inputStyle, fontFamily: 'var(--font-mono)' }} /></label>
      </div>

      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-xs" style={{ color: status?.message ? 'var(--warning)' : 'var(--text-muted)' }}>{status?.message ?? t('providers.lifecycleHint' as MessageKey)}</span>
        <button type="button" disabled={saving || loading} onClick={() => void save()} style={{ ...inputStyle, background: 'var(--accent)', borderColor: 'var(--accent)', color: '#fff' }}><Save size={14} aria-hidden="true" /> {saving ? t('common.saving' as MessageKey) : t('providers.lifecycleSave' as MessageKey)}</button>
      </div>

      {secretWarning && (
        <div className="grid gap-2 border p-3 text-xs" style={{ borderColor: 'var(--warning)', background: 'var(--warning-bg)' }} role="alert">
          <strong>{t('providers.lifecycleSecretWarningTitle' as MessageKey)}</strong>
          <span>{t('providers.lifecycleSecretWarningBody' as MessageKey)}</span>
          <div className="flex justify-end gap-2">
            <button type="button" onClick={() => setSecretWarning(false)} style={inputStyle}>{t('common.cancel' as MessageKey)}</button>
            <button type="button" disabled={secretCountdown > 0 || saving} onClick={() => void save()} style={{ ...inputStyle, background: 'var(--warning)', borderColor: 'var(--warning)', color: '#111' }}>{secretCountdown > 0 ? `${secretCountdown}s` : t('providers.lifecycleConfirmSecret' as MessageKey)}</button>
          </div>
        </div>
      )}
    </div>
  );
}
