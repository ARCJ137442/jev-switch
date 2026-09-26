import { useEffect, useRef, useState, type FormEvent } from 'react';
import {
  clearAdminSession,
  getStatus,
  loginAdmin,
  putListen,
  putMode,
  putPassword,
  type AdminStatus,
  type ModeResponse,
} from '../../api/admin';
import { useToast } from '../../app/feedback';
import { useI18n, type MessageKey } from '../../i18n';

type DialogState = { type: 'mode'; mode: AdminStatus['mode'] } | { type: 'password' } | null;

interface InstanceSettingsProps {
  status: AdminStatus | null;
  onStatusChange: (status: AdminStatus) => void;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function InstanceSettings({
  status,
  onStatusChange,
  open,
  onOpenChange,
}: InstanceSettingsProps) {
  const { t } = useI18n();
  const { toast } = useToast();
  const [listenAddress, setListenAddress] = useState(status?.bind ?? '');
  const [editingListen, setEditingListen] = useState(false);
  const [dialog, setDialog] = useState<DialogState>(null);
  const [secret, setSecret] = useState('');
  const [dialogError, setDialogError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const sectionRef = useRef<HTMLDetailsElement>(null);

  useEffect(() => setListenAddress(status?.bind ?? ''), [status?.bind]);

  useEffect(() => {
    if (open) sectionRef.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  }, [open]);

  useEffect(() => {
    const element = dialogRef.current;
    if (!element) return;
    if (dialog && !element.open) element.showModal();
    else if (!dialog && element.open) element.close();
  }, [dialog]);

  const copy = (key: string, vars?: Record<string, string | number>) =>
    t(key as MessageKey, vars);

  const openDialog = (next: Exclude<DialogState, null>) => {
    setSecret('');
    setDialogError(null);
    setDialog(next);
  };

  const closeDialog = () => {
    if (busy) return;
    setDialog(null);
    setSecret('');
    setDialogError(null);
  };

  const refreshStatus = async () => {
    const result = await getStatus();
    onStatusChange(result.status);
    return result.status;
  };

  const reportModeResult = (result: ModeResponse) => {
    const rebind = result.rebind;
    if ('skipped' in rebind) {
      toast('warn', copy('instance.modeRebindSkipped', { mode: result.mode, reason: rebind.skipped }));
    } else if (rebind.ok) {
      toast('ok', copy('instance.modeUpdated', { mode: result.mode, from: rebind.from, to: rebind.to }));
    } else {
      toast('warn', copy('instance.modeRebindFailed', {
        mode: result.mode,
        reason: rebind.reason ?? `${rebind.from} → ${rebind.to}`,
      }));
    }
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!dialog || busy || !status) return;
    setBusy(true);
    setDialogError(null);
    try {
      if (dialog.type === 'password') {
        const result = await putPassword(secret);
        clearAdminSession();
        const nextStatus = {
          ...status,
          password_set: true,
          env_override_active: result.env_override_active || status.env_override_active,
        };
        onStatusChange(nextStatus);
        setDialog(null);
        setSecret('');
        if (status.mode === 'cloud' && !result.env_override_active) {
          try {
            await loginAdmin(secret);
          } catch {
            // Password update succeeded; Shell will request login on the next admin call.
          }
        }
        toast(result.env_override_active ? 'warn' : 'ok', copy('instance.passwordSaved'));
        return;
      }

      const isFirstCloudPassword = dialog.mode === 'cloud' && !status.password_set;
      if (dialog.mode === 'cloud' && status.password_set) {
        // Verify the existing password and establish the cloud admin session before
        // flipping auth policy, so the current UI remains authorized after the switch.
        await loginAdmin(secret);
      }

      const result = await putMode({
        mode: dialog.mode,
        admin_password: isFirstCloudPassword ? secret : null,
      });
      reportModeResult(result);

      if (isFirstCloudPassword) {
        // put_mode sets the first password and invalidates any old sessions. Create
        // a fresh session with that same in-memory value for the next admin request.
        try {
          await loginAdmin(secret);
        } catch {
          toast('warn', copy('instance.cloudLoginAgain'));
        }
      }

      setDialog(null);
      setSecret('');
      await refreshStatus();
    } catch (error) {
      const reason = (error as Error).message;
      setDialogError(reason);
      if (dialog.type === 'mode') {
        toast('danger', copy('instance.modeFailed', { reason }));
      } else {
        toast('danger', copy('instance.passwordFailed', { reason }));
      }
    } finally {
      setBusy(false);
    }
  };

  const saveListen = async (address: string) => {
    if (!status || busy) return;
    setBusy(true);
    try {
      const result = await putListen(address.trim());
      setListenAddress(result.addr);
      setEditingListen(false);
      toast('ok', copy('instance.listenSaved', { addr: result.addr }));
      await refreshStatus();
    } catch (error) {
      toast('danger', copy('instance.listenFailed', { reason: (error as Error).message }));
    } finally {
      setBusy(false);
    }
  };

  const modeTarget = status?.mode === 'local' ? 'cloud' : 'local';
  const needsPassword = dialog?.type === 'mode' && dialog.mode === 'cloud' && !status?.password_set;
  const needsAuthentication = dialog?.type === 'mode' && dialog.mode === 'cloud' && !!status?.password_set;
  const panel: React.CSSProperties = {
    background: 'var(--surface)',
    border: '1px solid var(--border)',
    borderRadius: 'var(--radius)',
  };
  const button: React.CSSProperties = {
    border: '1px solid var(--border)',
    borderRadius: 'var(--radius)',
    background: 'var(--surface-hover)',
    color: 'var(--text)',
    padding: '0.45rem 0.75rem',
    fontSize: 'var(--text-sm)',
  };
  const field: React.CSSProperties = {
    border: '1px solid var(--border)',
    borderRadius: 'var(--radius)',
    background: 'var(--surface)',
    color: 'var(--text)',
    padding: '0.5rem 0.65rem',
    fontSize: 'var(--text-sm)',
  };

  return (
    <>
      <details
        id="instance-settings"
        ref={sectionRef}
        open={open}
        onToggle={(event) => onOpenChange(event.currentTarget.open)}
        className="fade-in mb-6"
        style={panel}
      >
        <summary className="cursor-pointer list-none px-5 py-4">
          <span className="font-semibold" style={{ fontSize: 'var(--text-lg)' }}>
            {copy('instance.settingsTitle')}
          </span>
          <span className="ml-3" style={{ fontSize: 'var(--text-sm)', color: 'var(--text-muted)' }}>
            {copy('instance.settingsSummary')}
          </span>
        </summary>

        <div className="grid gap-0 border-t md:grid-cols-2" style={{ borderColor: 'var(--border)' }}>
          <section className="space-y-3 p-5 md:border-r" style={{ borderColor: 'var(--border)' }}>
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <h2 className="font-semibold" style={{ fontSize: 'var(--text-base)' }}>{copy('instance.modeTitle')}</h2>
                <p className="mt-1" style={{ color: 'var(--text-muted)', fontSize: 'var(--text-sm)' }}>
                  {copy('instance.localHint')}
                </p>
              </div>
              <span className="rounded-full px-2.5 py-1 font-mono text-xs" style={{ background: 'var(--surface-hover)' }}>
                {status?.mode ?? '—'}
              </span>
            </div>
            <p style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>
              {copy('instance.modeHint')}
            </p>
            {status?.password_set === false && (
              <p role="status" style={{ color: 'var(--warning)', fontSize: 'var(--text-sm)' }}>
                {copy('instance.passwordMissing')}
              </p>
            )}
            <button
              type="button"
              disabled={!status || busy}
              onClick={() => status && openDialog({ type: 'mode', mode: modeTarget })}
              style={button}
            >
              {copy('instance.switchTo', { mode: modeTarget })}
            </button>
          </section>

          <section className="space-y-3 p-5">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <h2 className="font-semibold" style={{ fontSize: 'var(--text-base)' }}>{copy('instance.passwordSection')}</h2>
                <p className="mt-1" style={{ color: 'var(--text-muted)', fontSize: 'var(--text-sm)' }}>
                  {status?.password_set ? copy('instance.passwordConfigured') : copy('instance.passwordMissing')}
                </p>
              </div>
              <button type="button" disabled={!status || busy} onClick={() => openDialog({ type: 'password' })} style={button}>
                {copy(status?.password_set ? 'instance.changePassword' : 'instance.setPassword')}
              </button>
            </div>
            <p style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>{copy('instance.passwordHint')}</p>
          </section>

          <section className="space-y-3 border-t p-5 md:col-span-2" style={{ borderColor: 'var(--border)' }}>
            <div className="flex flex-wrap items-baseline justify-between gap-2">
              <h2 className="font-semibold" style={{ fontSize: 'var(--text-base)' }}>{copy('instance.listenTitle')}</h2>
              <code className="font-mono text-xs" style={{ color: 'var(--text-muted)' }}>{status?.bind ?? '—'}</code>
            </div>
            {editingListen ? (
              <form
                className="flex flex-wrap items-center gap-2"
                onSubmit={(event) => { event.preventDefault(); void saveListen(listenAddress); }}
              >
                <label className="sr-only" htmlFor="instance-listen-address">{copy('instance.listenLabel')}</label>
                <input
                  id="instance-listen-address"
                  value={listenAddress}
                  onChange={(event) => setListenAddress(event.target.value)}
                  placeholder={status?.bind ?? '127.0.0.1:11435'}
                  spellCheck={false}
                  disabled={busy}
                  style={{ ...field, minWidth: '16rem', flex: '1 1 18rem', fontFamily: 'var(--font-mono)' }}
                />
                <button type="submit" disabled={busy || listenAddress.trim().length === 0} style={button}>
                  {busy ? copy('instance.saving') : copy('instance.listenSave')}
                </button>
                <button type="button" disabled={busy} onClick={() => setEditingListen(false)} style={button}>
                  {copy('instance.cancel')}
                </button>
                <button type="button" disabled={busy} onClick={() => void saveListen('auto')} style={button}>
                  {copy('instance.listenAuto')}
                </button>
              </form>
            ) : (
              <div className="flex flex-wrap items-center gap-2">
                <button type="button" disabled={!status || busy} onClick={() => setEditingListen(true)} style={button}>
                  {copy('instance.listenEdit')}
                </button>
                <button type="button" disabled={!status || busy} onClick={() => void saveListen('auto')} style={button}>
                  {copy('instance.listenAuto')}
                </button>
              </div>
            )}
            <p style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>{copy('instance.listenHint')}</p>
          </section>

          {status?.env_override_active && (
            <p className="border-t px-5 py-3 md:col-span-2" role="status" style={{ borderColor: 'var(--border)', color: 'var(--warning)', fontSize: 'var(--text-xs)' }}>
              {copy('instance.envOverride')}
            </p>
          )}
        </div>
      </details>

      <dialog
        ref={dialogRef}
        onCancel={(event) => { event.preventDefault(); closeDialog(); }}
        onClose={() => { if (dialog) closeDialog(); }}
        className="responsive-dialog"
        style={{ ...panel, color: 'var(--text)', width: 'min(30rem, calc(100vw - 2rem))', padding: 0 }}
      >
        {dialog && (
          <form onSubmit={(event) => void submit(event)} className="space-y-4 p-5">
            <div>
              <h2 className="font-semibold" style={{ fontSize: 'var(--text-lg)' }}>
                {dialog.type === 'mode'
                  ? copy('instance.confirmModeTitle')
                  : copy(status?.password_set ? 'instance.changePassword' : 'instance.passwordActionTitle')}
              </h2>
              <p className="mt-2" style={{ color: 'var(--text-muted)', fontSize: 'var(--text-sm)' }}>
                {dialog.type === 'mode'
                  ? needsPassword
                    ? copy('instance.cloudPasswordRequired')
                    : copy('instance.confirmModeBody', { mode: dialog.mode })
                  : copy('instance.passwordActionBody')}
              </p>
              {needsAuthentication && (
                <p className="mt-2" style={{ color: 'var(--text-muted)', fontSize: 'var(--text-sm)' }}>
                  {copy('instance.authPrompt')}
                </p>
              )}
            </div>

            {(needsPassword || needsAuthentication || dialog.type === 'password') && (
              <>
                <label className="block space-y-1" style={{ fontSize: 'var(--text-sm)' }}>
                  <span>{copy(needsAuthentication ? 'instance.currentPassword' : 'instance.newPassword')}</span>
                  <input
                    type="password"
                    value={secret}
                    onChange={(event) => setSecret(event.target.value)}
                    autoComplete={needsAuthentication ? 'current-password' : 'new-password'}
                    autoFocus
                    required
                    disabled={busy}
                    style={{ ...field, width: '100%' }}
                  />
                </label>
                <p style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>{copy('instance.passwordHint')}</p>
              </>
            )}

            {dialogError && <p role="alert" style={{ color: 'var(--danger)', fontSize: 'var(--text-sm)' }}>{dialogError}</p>}
            <div className="flex flex-wrap justify-end gap-2">
              <button type="button" disabled={busy} onClick={closeDialog} style={button}>{copy('instance.cancel')}</button>
              <button
                type="submit"
                disabled={busy || ((needsPassword || needsAuthentication || dialog.type === 'password') && !secret.trim())}
                style={{ ...button, background: 'var(--accent)', borderColor: 'var(--accent)', color: '#fff' }}
              >
                {busy ? copy('instance.saving') : dialog.type === 'mode' ? copy('instance.confirm') : copy(status?.password_set ? 'instance.changePassword' : 'instance.setPassword')}
              </button>
            </div>
          </form>
        )}
      </dialog>
    </>
  );
}
