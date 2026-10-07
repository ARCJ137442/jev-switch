import { useEffect, useMemo, useState } from 'react';
import { LoaderCircle, Play, RefreshCw, Save, Square, Terminal } from 'lucide-react';
import {
  getProviderLifecycle,
  providerLifecycleStatus,
  putProviderLifecycle,
  startProviderService,
  stopProviderService,
  AdminApiError,
  type AdminProvider,
} from '../../api/admin';
import type { ProviderLifecycleConfig } from '../../generated/ProviderLifecycleConfig';
import type { LifecycleStatus } from '../../generated/LifecycleStatus';
import { useI18n, type MessageKey } from '../../i18n';
import { useToast } from '../../app/feedback';

interface Props {
  provider: AdminProvider;
  busy: boolean;
  autoProbe: boolean;
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

function statusLabel(state: string | undefined, translate: (key: MessageKey) => string): string {
  const keyByState: Record<string, MessageKey> = {
    running: 'providers.lifecycleStateRunning' as MessageKey,
    starting: 'providers.lifecycleStateStarting' as MessageKey,
    stopping: 'providers.lifecycleStateStopping' as MessageKey,
    stopped: 'providers.lifecycleStateStopped' as MessageKey,
    uncontrolled: 'providers.lifecycleStateUncontrolled' as MessageKey,
    disabled: 'providers.lifecycleStateDisabled' as MessageKey,
    unknown: 'providers.lifecycleStateUnknown' as MessageKey,
    failed: 'providers.lifecycleStateFailed' as MessageKey,
  };
  const key = state ? keyByState[state] : undefined;
  return key ? translate(key) : state ?? translate('providers.lifecycleStateUnknown' as MessageKey);
}

type LifecycleFailure = {
  message: string;
  code?: string;
  remediation?: string;
  detail?: string;
  requestId?: string;
};

function describeFailure(error: unknown): LifecycleFailure {
  if (error instanceof AdminApiError) {
    return {
      message: error.message,
      code: error.code,
      remediation: error.remediation,
      detail: error.detail,
      requestId: error.requestId,
    };
  }
  return { message: error instanceof Error ? error.message : String(error) };
}

const lifecycleErrorCopy: Record<string, { message: MessageKey; remediation: MessageKey }> = {
  invalid_lifecycle_body: {
    message: 'providers.lifecycleErrorInvalidBody' as MessageKey,
    remediation: 'providers.lifecycleErrorInvalidBodyRemediation' as MessageKey,
  },
  invalid_lifecycle_configuration: {
    message: 'providers.lifecycleErrorInvalidConfig' as MessageKey,
    remediation: 'providers.lifecycleErrorInvalidConfigRemediation' as MessageKey,
  },
  host_commands_disabled: {
    message: 'providers.lifecycleErrorHostCommandsDisabled' as MessageKey,
    remediation: 'providers.lifecycleErrorHostCommandsDisabledRemediation' as MessageKey,
  },
  provider_not_controllable: {
    message: 'providers.lifecycleErrorNotControllable' as MessageKey,
    remediation: 'providers.lifecycleErrorNotControllableRemediation' as MessageKey,
  },
  command_not_configured: {
    message: 'providers.lifecycleErrorCommandMissing' as MessageKey,
    remediation: 'providers.lifecycleErrorCommandMissingRemediation' as MessageKey,
  },
  stop_not_configured: {
    message: 'providers.lifecycleErrorStopMissing' as MessageKey,
    remediation: 'providers.lifecycleErrorStopMissingRemediation' as MessageKey,
  },
  command_failed: {
    message: 'providers.lifecycleErrorCommandFailed' as MessageKey,
    remediation: 'providers.lifecycleErrorCommandFailedRemediation' as MessageKey,
  },
  program_path_must_be_absolute: {
    message: 'providers.lifecycleErrorAbsolutePath' as MessageKey,
    remediation: 'providers.lifecycleErrorAbsolutePathRemediation' as MessageKey,
  },
  readiness_timeout: {
    message: 'providers.lifecycleErrorReadinessTimeout' as MessageKey,
    remediation: 'providers.lifecycleErrorReadinessTimeoutRemediation' as MessageKey,
  },
  readiness_process_exited: {
    message: 'providers.lifecycleErrorProcessExited' as MessageKey,
    remediation: 'providers.lifecycleErrorProcessExitedRemediation' as MessageKey,
  },
};

function localizeFailure(failure: LifecycleFailure, translate: (key: MessageKey) => string): LifecycleFailure {
  const copy = failure.code ? lifecycleErrorCopy[failure.code] : undefined;
  return copy
    ? { ...failure, message: translate(copy.message), remediation: translate(copy.remediation) }
    : failure;
}

function commandPathIsAbsolute(value: string): boolean {
  return value.startsWith('/') || /^[A-Za-z]:[\\/]/.test(value) || value.startsWith('\\\\');
}

export function ProviderLifecyclePanel({ provider, busy, autoProbe }: Props) {
  const { t } = useI18n();
  const { toast } = useToast();
  const [command, setCommand] = useState<ProviderLifecycleConfig>(() => defaultCommand(provider));
  const [savedCommand, setSavedCommand] = useState<ProviderLifecycleConfig | null>(null);
  const [status, setStatus] = useState<LifecycleStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [action, setAction] = useState<'start' | 'stop' | 'status' | null>(null);
  const [secretWarning, setSecretWarning] = useState(false);
  const [secretCountdown, setSecretCountdown] = useState(5);
  const [failure, setFailure] = useState<LifecycleFailure | null>(null);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setFailure(null);
    setSavedCommand(null);
    void getProviderLifecycle(provider.id)
      .then((doc) => {
        if (!active) return;
        setCommand(doc.command);
        setSavedCommand(doc.command);
        setStatus(doc.status);
      })
      .catch((error) => {
        if (active) {
          const next = localizeFailure(describeFailure(error), (key) => t(key));
          setFailure(next);
          toast('danger', t('providers.lifecycleLoadFailed' as MessageKey, { reason: next.message }));
        }
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

  useEffect(() => {
    if (!autoProbe || loading || action !== null) return;
    let active = true;
    const poll = async () => {
      try {
        const next = await providerLifecycleStatus(provider.id);
        if (active) setStatus(next);
      } catch (error) {
        if (active) setFailure(localizeFailure(describeFailure(error), (key) => t(key)));
      }
    };
    const timer = window.setInterval(() => void poll(), status && ['starting', 'stopping'].includes(status.state) ? 1000 : 10000);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [action, autoProbe, loading, provider.id, status, t]);

  const commandFields = useMemo(() => ({
    args: command.args.join('\n'),
    stop_args: command.stop_args.join('\n'),
    status_args: command.status_args.join('\n'),
  }), [command.args, command.stop_args, command.status_args]);

  const update = <K extends keyof ProviderLifecycleConfig>(key: K, value: ProviderLifecycleConfig[K]) => {
    setFailure(null);
    setCommand((current) => ({ ...current, [key]: value }));
  };

  const dirty = savedCommand !== null && JSON.stringify(command) !== JSON.stringify(savedCommand);
  const operationBusy = status?.state === 'starting' || status?.state === 'stopping';

  const validateBeforeSave = (): string | null => {
    if ((command.controllable || command.mode !== 'manual' || command.args.length > 0) && !command.program?.trim()) {
      return t('providers.lifecycleProgramRequired' as MessageKey);
    }
    if (command.program?.trim() && !commandPathIsAbsolute(command.program.trim())) {
      return t('providers.lifecycleProgramAbsolute' as MessageKey);
    }
    if (command.stop_program?.trim() && !commandPathIsAbsolute(command.stop_program.trim())) {
      return t('providers.lifecycleStopProgramAbsolute' as MessageKey);
    }
    if (command.status_program?.trim() && !commandPathIsAbsolute(command.status_program.trim())) {
      return t('providers.lifecycleStatusProgramAbsolute' as MessageKey);
    }
    return null;
  };

  const save = async () => {
    const validationError = validateBeforeSave();
    if (validationError) {
      const next = { message: validationError };
      setFailure(next);
      toast('danger', validationError);
      return;
    }
    if (command.inject_api_key && secretCountdown > 0) {
      setSecretWarning(true);
      return;
    }
    setSaving(true);
    setFailure(null);
    try {
      const doc = await putProviderLifecycle(provider.id, command);
      setCommand(doc.command);
      setSavedCommand(doc.command);
      setStatus(doc.status);
      setSecretWarning(false);
      toast('ok', t('providers.lifecycleSaved' as MessageKey));
    } catch (error) {
      const next = localizeFailure(describeFailure(error), (key) => t(key));
      setFailure(next);
      toast('danger', t('providers.lifecycleSaveFailed' as MessageKey, { reason: next.message }));
    } finally {
      setSaving(false);
    }
  };

  const run = async (kind: 'start' | 'stop' | 'status') => {
    if ((kind === 'start' || kind === 'stop') && dirty) {
      const next = { message: t('providers.lifecycleSaveBeforeAction' as MessageKey) };
      setFailure(next);
      toast('warn', next.message);
      return;
    }
    setAction(kind);
    setFailure(null);
    try {
      const next = kind === 'start'
        ? await startProviderService(provider.id)
        : kind === 'stop'
          ? await stopProviderService(provider.id)
          : await providerLifecycleStatus(provider.id);
      setStatus(next);
      toast(next.state === 'running' ? 'ok' : ['unknown', 'starting', 'stopping'].includes(next.state) ? 'warn' : 'danger', t(`providers.lifecycle${kind[0].toUpperCase()}${kind.slice(1)}Result` as MessageKey, { state: statusLabel(next.state, (key) => t(key)) }));
    } catch (error) {
      const next = localizeFailure(describeFailure(error), (key) => t(key));
      setFailure(next);
      toast('danger', t('providers.lifecycleActionFailed' as MessageKey, { reason: next.message }));
    } finally {
      setAction(null);
    }
  };

  return (
    <div className="mt-3 grid gap-3 border-t pt-3" style={{ borderColor: 'var(--border)' }} aria-busy={loading}>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Terminal size={15} aria-hidden="true" style={{ color: 'var(--accent)' }} />
          <strong style={{ fontSize: 'var(--text-sm)' }}>{t('providers.lifecycleTitle' as MessageKey)}</strong>
          <span className="font-mono text-xs" style={{ color: dirty ? 'var(--warning)' : statusTone(status) }}>
            {dirty ? t('providers.lifecycleUnsaved' as MessageKey) : loading ? '…' : statusLabel(status?.state, (key) => t(key))}
          </span>
        </div>
        <div className="flex flex-wrap gap-1.5">
          <button type="button" aria-busy={action === 'status'} disabled={busy || loading || action !== null} onClick={() => void run('status')} style={{ ...inputStyle, opacity: action !== null && action !== 'status' ? 0.55 : 1 }} title={t('providers.lifecycleCheckHint' as MessageKey)}>{action === 'status' ? <LoaderCircle size={14} className="animate-spin" aria-hidden="true" /> : <RefreshCw size={14} aria-hidden="true" />} {action === 'status' ? t('providers.lifecycleChecking' as MessageKey) : loading ? t('providers.lifecycleLoading' as MessageKey) : t('providers.lifecycleCheck' as MessageKey)}</button>
          <button type="button" aria-busy={action === 'start' || (operationBusy && status?.state === 'starting')} disabled={busy || loading || action !== null || operationBusy || !command.controllable || dirty} onClick={() => void run('start')} style={{ ...inputStyle, borderColor: 'var(--success)', opacity: action !== null && action !== 'start' ? 0.55 : 1 }} title={dirty ? t('providers.lifecycleSaveBeforeAction' as MessageKey) : undefined}>{action === 'start' || (operationBusy && status?.state === 'starting') ? <LoaderCircle size={14} className="animate-spin" aria-hidden="true" /> : <Play size={14} aria-hidden="true" />} {action === 'start' || (operationBusy && status?.state === 'starting') ? t('providers.lifecycleStarting' as MessageKey) : t('providers.lifecycleStart' as MessageKey)}</button>
          <button type="button" aria-busy={action === 'stop' || (operationBusy && status?.state === 'stopping')} disabled={busy || loading || action !== null || operationBusy || !command.controllable || dirty} onClick={() => void run('stop')} style={{ ...inputStyle, borderColor: 'var(--danger)', opacity: action !== null && action !== 'stop' ? 0.55 : 1 }} title={dirty ? t('providers.lifecycleSaveBeforeAction' as MessageKey) : undefined}>{action === 'stop' || (operationBusy && status?.state === 'stopping') ? <LoaderCircle size={14} className="animate-spin" aria-hidden="true" /> : <Square size={14} aria-hidden="true" />} {action === 'stop' || (operationBusy && status?.state === 'stopping') ? t('providers.lifecycleStopping' as MessageKey) : t('providers.lifecycleStop' as MessageKey)}</button>
        </div>
      </div>

      {loading && <div className="border px-3 py-2 text-xs" role="status" style={{ color: 'var(--text-muted)', background: 'var(--surface-hover)' }}>{t('providers.lifecycleLoadingDetail' as MessageKey)}</div>}
      <fieldset disabled={loading || saving} className="contents">
      <div className="grid gap-3 md:grid-cols-2" style={{ opacity: loading ? 0.6 : 1 }}>
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
        <span className="text-xs" style={{ color: status?.message || dirty ? 'var(--warning)' : 'var(--text-muted)' }}>
          {dirty ? t('providers.lifecycleSaveBeforeAction' as MessageKey) : status?.message ?? t('providers.lifecycleHint' as MessageKey)}
        </span>
        <button type="button" disabled={saving || loading || !dirty} onClick={() => void save()} style={{ ...inputStyle, background: dirty ? 'var(--accent)' : 'var(--surface-hover)', borderColor: dirty ? 'var(--accent)' : 'var(--border)', color: dirty ? '#fff' : 'var(--text-muted)' }}><Save size={14} aria-hidden="true" /> {saving ? t('common.saving' as MessageKey) : t('providers.lifecycleSave' as MessageKey)}</button>
      </div>

      {failure && (
        <div className="grid gap-1 border p-3 text-xs" style={{ borderColor: 'var(--danger)', background: 'var(--danger-bg)' }} role="alert">
          <strong>{t('providers.lifecycleErrorTitle' as MessageKey)}</strong>
          <span>{failure.message}</span>
          {failure.code && <span className="font-mono">{t('providers.lifecycleErrorCode' as MessageKey, { code: failure.code })}</span>}
          {failure.detail && <span>{failure.detail}</span>}
          {failure.remediation && <span>{t('providers.lifecycleErrorRemediation' as MessageKey, { remediation: failure.remediation })}</span>}
          {failure.requestId && <span className="font-mono">{t('providers.lifecycleErrorRequestId' as MessageKey, { id: failure.requestId })}</span>}
        </div>
      )}

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
      </fieldset>
    </div>
  );
}
