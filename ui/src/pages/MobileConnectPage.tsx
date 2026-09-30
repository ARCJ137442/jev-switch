import { useState, type FormEvent } from 'react';
import { Link2 } from 'lucide-react';
import { useI18n, type MessageKey } from '../i18n';
import { saveApiBasePreference } from '../api/base';

export function MobileConnectPage({ onConnected, onUseLocalGateway }: { onConnected: () => void; onUseLocalGateway: () => void }) {
  const { t } = useI18n();
  const [address, setAddress] = useState('');
  const [error, setError] = useState<string | null>(null);
  const submit = (event: FormEvent) => {
    event.preventDefault();
    try {
      saveApiBasePreference(address);
      setError(null);
      onConnected();
    } catch {
      setError(t('settings.addressInvalid' as MessageKey));
    }
  };
  return <main className="min-h-dvh px-5 py-8" style={{ background: 'var(--bg)', color: 'var(--text)' }}>
    <section className="mx-auto mt-10 grid w-full max-w-lg gap-5 border p-5" style={{ background: 'var(--surface)', borderColor: 'var(--border)', borderRadius: 'var(--radius)' }}>
      <span className="inline-flex items-center gap-2 text-xs font-semibold" style={{ color: 'var(--warning)' }}><span className="h-2 w-2 rounded-full" style={{ background: 'var(--warning)' }}/>{t('mobile.experimental' as MessageKey)}</span>
      <div><h1 className="font-semibold" style={{ fontSize: 'var(--text-xl)' }}>{t('mobile.connectTitle' as MessageKey)}</h1><p className="mt-2 text-sm" style={{ color: 'var(--text-muted)' }}>{t('mobile.connectHint' as MessageKey)}</p></div>
      <form className="grid gap-3" onSubmit={submit}><label className="grid gap-2 text-sm"><span>{t('mobile.address' as MessageKey)}</span><input type="url" required value={address} onChange={(event) => setAddress(event.target.value)} placeholder={t('settings.apiAddressPlaceholder' as MessageKey)} className="min-h-11 border px-3" style={{ background: 'var(--surface)', borderColor: 'var(--border)', borderRadius: 'var(--radius)', color: 'var(--text)' }}/></label>{error && <p role="alert" className="text-sm" style={{ color: 'var(--danger)' }}>{error}</p>}<button type="submit" className="inline-flex min-h-11 items-center justify-center gap-2 font-semibold text-white" style={{ background: 'var(--accent)', borderRadius: 'var(--radius)' }}><Link2 size={16}/>{t('mobile.connect' as MessageKey)}</button></form>
      <div className="flex items-center gap-3 text-xs" style={{ color: 'var(--text-subtle)' }}><span className="h-px flex-1" style={{ background: 'var(--border)' }}/>{t('mobile.or' as MessageKey)}<span className="h-px flex-1" style={{ background: 'var(--border)' }}/></div>
      <button type="button" className="min-h-11 border px-3 text-sm font-semibold" style={{ borderColor: 'var(--border)', borderRadius: 'var(--radius)', color: 'var(--text)' }} onClick={() => { saveApiBasePreference('http://127.0.0.1:11435'); onUseLocalGateway(); }}>{t('mobile.useLocal' as MessageKey)}</button>
    </section>
  </main>;
}
