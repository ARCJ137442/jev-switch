import { useEffect, useMemo, useState } from 'react';
import { Search } from 'lucide-react';
import { getStatus, type AdminStatus } from '../api/admin';
import { clearApiBasePreference, readApiBasePreference, saveApiBasePreference } from '../api/base';
import { useAuth } from '../auth/AuthContext';
import { InstanceSettings } from '../components/settings/InstanceSettings';
import { useI18n, type MessageKey } from '../i18n';
import { supportedLanguages } from '../i18n/languages';
import { getLangPreference } from '../i18n/core';
import { applyThemePreference, readThemePreference, type ThemePreference } from '../theme';
import { readAutoProviderProbe, SETTINGS_CHANGE_EVENT, writeAutoProviderProbe } from '../settings/preferences';
import { getAndroidDebugLogStatus, isAndroidTauriRuntime, setAndroidDebugLog, type AndroidDebugLogStatus } from '../api/androidDebug';

const fieldClass = 'min-h-10 border px-3 text-sm';
const fieldStyle: React.CSSProperties = { borderColor: 'var(--border)', borderRadius: 'var(--radius)', background: 'var(--surface)', color: 'var(--text)' };

export function SettingsPage() {
  const { t, lang, setLang, setSystemLang } = useI18n();
  const auth = useAuth();
  const [query, setQuery] = useState('');
  const [theme, setTheme] = useState<ThemePreference>(readThemePreference);
  const [language, setLanguage] = useState(() => getLangPreference());
  const [autoProbe, setAutoProbe] = useState(readAutoProviderProbe);
  const [apiBase, setApiBase] = useState(() => readApiBasePreference() ?? '');
  const [saved, setSaved] = useState(false);
  const [status, setStatus] = useState<AdminStatus | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [debugLog, setDebugLog] = useState<AndroidDebugLogStatus | null>(null);
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const matches = (text: string) => !normalizedQuery || text.toLocaleLowerCase().includes(normalizedQuery);
  const availableLanguages = useMemo(() => supportedLanguages, []);

  useEffect(() => {
    let alive = true;
    getStatus().then((result) => { if (alive) setStatus(result.status); }).catch((error: unknown) => {
      if (alive) setStatusError(error instanceof Error ? error.message : String(error));
    });
    if (isAndroidTauriRuntime()) getAndroidDebugLogStatus().then((result) => { if (alive) setDebugLog(result); }).catch(() => undefined);
    const syncProbe = () => setAutoProbe(readAutoProviderProbe());
    window.addEventListener(SETTINGS_CHANGE_EVENT, syncProbe);
    return () => { alive = false; window.removeEventListener(SETTINGS_CHANGE_EVENT, syncProbe); };
  }, []);

  const saveApiAddress = () => {
    try {
      if (!apiBase.trim()) clearApiBasePreference();
      else saveApiBasePreference(apiBase);
      setApiBase(readApiBasePreference() ?? '');
      setSaved(true);
      window.setTimeout(() => setSaved(false), 1800);
      window.dispatchEvent(new Event('jev-api-base-change'));
    } catch {
      setStatusError(t('settings.addressInvalid' as MessageKey));
    }
  };

  const groups = [
    { id: 'appearance', label: t('settings.appearance' as MessageKey), terms: 'appearance theme language 外观 主题 语言', content: (
      <div className="grid gap-4 sm:grid-cols-2">
        {matches('theme 主题 appearance 外观') && <label className="grid gap-2 text-sm">
          <span>{t('settings.theme' as MessageKey)}</span>
          <select className={fieldClass} style={fieldStyle} value={theme} onChange={(event) => { const next = event.target.value as ThemePreference; setTheme(next); applyThemePreference(next); }}>
            <option value="system">{t('settings.themeSystem' as MessageKey)}</option>
            <option value="light">{t('settings.themeLight' as MessageKey)}</option>
            <option value="dark">{t('settings.themeDark' as MessageKey)}</option>
          </select>
        </label>}
        {matches('language locale 语言') && <label className="grid gap-2 text-sm">
          <span>{t('settings.language' as MessageKey)}</span>
          <select className={fieldClass} style={fieldStyle} value={language} onChange={(event) => {
            const next = event.target.value;
            if (next === 'system') { setSystemLang(); setLanguage('system'); }
            else { setLang(next as typeof lang); setLanguage(next as typeof lang); }
          }}>
            <option value="system">{t('settings.themeSystem' as MessageKey)}</option>
            {availableLanguages.map((item) => <option key={item.code} value={item.code}>{item.label}</option>)}
          </select>
        </label>}
      </div>
    ) },
    { id: 'providers', label: t('settings.probes' as MessageKey), terms: 'provider upstream probe health frequency 提供商 上游 探测 连通性 频率', content: (
      matches('provider upstream probe health frequency 提供商 上游 探测 连通性 频率') && <label className="flex items-start gap-3 text-sm">
        <input type="checkbox" checked={autoProbe} onChange={(event) => { const value = event.target.checked; setAutoProbe(value); writeAutoProviderProbe(value); }} className="mt-1 h-4 w-4 accent-[var(--accent)]" />
        <span className="grid gap-1"><strong>{t('settings.autoProbe' as MessageKey)}</strong><span style={{ color: 'var(--text-muted)' }}>{t('settings.autoProbeHint' as MessageKey)}</span></span>
      </label>
    ) },
    { id: 'connection', label: t('settings.apiAddress' as MessageKey), terms: 'gateway api url address endpoint connection 网关 API 地址 连接', content: (
      matches('gateway api url address endpoint connection 网关 API 地址 连接') && <div className="grid gap-2">
        <label className="grid gap-2 text-sm"><span>{t('settings.apiAddress' as MessageKey)}</span><input className={fieldClass} style={fieldStyle} type="url" value={apiBase} onChange={(event) => setApiBase(event.target.value)} placeholder={t('settings.apiAddressPlaceholder' as MessageKey)} /></label>
        <p className="text-xs" style={{ color: 'var(--text-muted)' }}>{t('settings.apiAddressHint' as MessageKey)}</p>
        <div className="flex flex-wrap gap-2"><button type="button" className={fieldClass} style={fieldStyle} onClick={saveApiAddress}>{saved ? t('settings.addressSaved' as MessageKey) : t('settings.saveAddress' as MessageKey)}</button><button type="button" className={fieldClass} style={fieldStyle} onClick={() => { setApiBase(''); clearApiBasePreference(); window.dispatchEvent(new Event('jev-api-base-change')); }}>{t('settings.resetAddress' as MessageKey)}</button></div>
      </div>
    ) },
    ...(isAndroidTauriRuntime() ? [{ id: 'android-debug', label: t('settings.androidDebug' as MessageKey), terms: 'android debug log diagnostics 安卓 调试 日志 诊断', content: (
      matches('android debug log diagnostics 安卓 调试 日志 诊断') && <label className="flex items-start gap-3 text-sm">
        <input type="checkbox" checked={debugLog?.enabled === true} onChange={async (event) => {
          try { setDebugLog(await setAndroidDebugLog(event.target.checked)); }
          catch (error) { setStatusError(error instanceof Error ? error.message : String(error)); }
        }} className="mt-1 h-4 w-4 accent-[var(--accent)]" />
        <span className="grid gap-1"><strong>{t('settings.androidDebugToggle' as MessageKey)}</strong><span style={{ color: 'var(--text-muted)' }}>{t('settings.androidDebugHint' as MessageKey)}</span>{debugLog?.path && <code className="break-all text-xs" style={{ color: 'var(--text-subtle)' }}>{debugLog.path}</code>}</span>
      </label>
    ) }] : []),
  ];
  const visibleGroups = groups.filter((group) => matches(`${group.label} ${group.terms}`));
  const showInstance = matches('instance mode listen password port 运行模式 监听 密码 端口');

  return <div className="page-container mx-auto w-full min-w-0">
    <div className="mb-5 flex flex-wrap items-center justify-between gap-3"><h1 className="font-semibold" style={{ fontSize: 'var(--text-2xl)' }}>{t('settings.title' as MessageKey)}</h1><label className="flex min-h-10 w-full max-w-md items-center gap-2 border px-3" style={fieldStyle}><Search size={16} aria-hidden="true"/><span className="sr-only">{t('settings.search' as MessageKey)}</span><input autoFocus type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t('settings.search' as MessageKey)} aria-label={t('settings.search' as MessageKey)} className="min-w-0 flex-1 bg-transparent outline-none"/></label></div>
    <div className="space-y-7">
      {visibleGroups.map((group) => <section key={group.id} className="border-b pb-6" style={{ borderColor: 'var(--border)' }}><h2 className="mb-4 font-semibold" style={{ fontSize: 'var(--text-lg)' }}>{group.label}</h2>{group.content}</section>)}
      {showInstance && auth.canManage && <section className="border-b pb-6" style={{ borderColor: 'var(--border)' }}><h2 className="mb-4 font-semibold" style={{ fontSize: 'var(--text-lg)' }}>{t('instance.manage' as MessageKey)}</h2>{statusError && <p role="alert" style={{ color: 'var(--danger)' }}>{statusError}</p>}<InstanceSettings status={status} onStatusChange={setStatus} open onOpenChange={() => undefined}/></section>}
      {visibleGroups.length === 0 && !showInstance && <p className="py-8 text-center text-sm" style={{ color: 'var(--text-muted)' }}>{t('settings.noResults' as MessageKey)}</p>}
      {auth.isReadOnly && <p className="text-sm" style={{ color: 'var(--text-muted)' }}>{t('settings.adminOnly' as MessageKey)}</p>}
    </div>
  </div>;
}
