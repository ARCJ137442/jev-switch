import { useEffect, useMemo, useRef, useState } from 'react';
import { CheckCircle2, Download, ExternalLink, GitBranch, Info, PackageCheck, RefreshCw, Search, Upload } from 'lucide-react';
import { fetchHealth, type HealthBody } from '../api';
import { getStatus, type AdminStatus } from '../api/admin';
import { clearApiBasePreference, normalizeApiBase, readApiBasePreference, saveApiBasePreference } from '../api/base';
import { checkLatestRelease, GITHUB_RELEASES_URL, GITHUB_REPOSITORY_URL, type ReleaseCheckResult } from '../api/releases';
import { exportRuntimeConfigJson, importRuntimeConfigJson } from '../api/configFile';
import { useAuth } from '../auth/AuthContext';
import { InstanceSettings } from '../components/settings/InstanceSettings';
import { SearchMark } from '../components/settings/SearchMark';
import { settingMatches } from '../components/settings/settingsSearch';
import { useI18n, type MessageKey } from '../i18n';
import { supportedLanguages } from '../i18n/languages';
import { getLangPreference } from '../i18n/core';
import { applyThemePreference, readThemePreference, type ThemePreference } from '../theme';
import { readAutoProviderProbe, readRoutingHudAutoHide, readShowStatusBar, readUiScale, SETTINGS_CHANGE_EVENT, UI_SCALE_MAX_PERCENT, UI_SCALE_MIN_PERCENT, UI_SCALE_STEP_PERCENT, writeAutoProviderProbe, writeRoutingHudAutoHide, writeShowStatusBar, writeUiScale } from '../settings/preferences';
import { createSettingsBackup, parseSettingsBackup } from '../settings/backup';
import { isAndroidTauriRuntime } from '../api/androidDebug';
import { AndroidDiagnostics } from '../components/settings/AndroidDiagnostics';
import { getAndroidForegroundServiceActive, getAndroidKeepaliveNotificationStatus, getAndroidNotificationPermissionState, requestAndroidNotificationPermission, setAndroidKeepaliveNotification, type AndroidNotificationPermissionState } from '../api/androidGateway';
import { getGatewayServiceStatus } from '../api/gatewayControl';
import { saveJsonFile } from '../api/nativeTransfer';
import pkg from '../../package.json';

const fieldClass = 'min-h-10 border px-3 text-sm';
const fieldStyle: React.CSSProperties = { borderColor: 'var(--border)', borderRadius: 'var(--radius)', background: 'var(--surface)', color: 'var(--text)' };
const groupSearchKeys: Record<string, MessageKey[]> = {
  personalization: ['settings.personalization', 'settings.dashboardLayout', 'settings.dashboardLayoutHint'],
  appearance: ['settings.theme', 'settings.themeSystem', 'settings.themeLight', 'settings.themeDark', 'settings.language', 'settings.showStatusBar', 'settings.showStatusBarHint', 'settings.uiScale', 'settings.uiScaleHint', 'settings.autoHideRoutingHud', 'settings.autoHideRoutingHudHint'],
  providers: ['settings.autoProbe', 'settings.autoProbeHint'],
  connection: ['settings.apiAddress', 'settings.apiAddressHint', 'settings.androidApiAddressHint', 'settings.saveAddress', 'settings.resetAddress'],
  'settings-backup': ['settings.backup', 'settings.backupHint', 'settings.exportBackup', 'settings.importBackup'],
  'android-debug': ['settings.androidDebug'],
  'android-keepalive': ['settings.androidKeepalive', 'settings.androidKeepaliveToggle', 'settings.androidKeepaliveHint', 'settings.androidNotificationPermission', 'settings.androidServiceStateHint'],
  'build-info': ['settings.buildInfo', 'settings.appVersion', 'settings.backendVersion', 'settings.apiRevision'],
  about: ['settings.about', 'settings.aboutHint', 'settings.checkUpdates'],
};
const instanceSearchKeys: MessageKey[] = [
  'instance.settingsTitle', 'instance.settingsSummary', 'instance.modeTitle', 'instance.localHint',
  'instance.modeHint', 'instance.lanTitle', 'instance.lanHint', 'instance.passwordSection',
  'instance.passwordHint', 'instance.listenTitle', 'instance.listenHint',
];

export function SettingsPage() {
  const { t, lang, setLang, setSystemLang } = useI18n();
  const auth = useAuth();
  const [query, setQuery] = useState('');
  const [searchFocused, setSearchFocused] = useState(false);
  const [theme, setTheme] = useState<ThemePreference>(readThemePreference);
  const [language, setLanguage] = useState(() => getLangPreference());
  const [autoProbe, setAutoProbe] = useState(readAutoProviderProbe);
  const [showStatusBar, setShowStatusBar] = useState(readShowStatusBar);
  const [uiScale, setUiScale] = useState(readUiScale);
  const [autoHideRoutingHud, setAutoHideRoutingHud] = useState(readRoutingHudAutoHide);
  const [apiBase, setApiBase] = useState(() => readApiBasePreference() ?? '');
  const [saved, setSaved] = useState(false);
  const [status, setStatus] = useState<AdminStatus | null>(null);
  const [health, setHealth] = useState<HealthBody | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [keepaliveNotificationEnabled, setKeepaliveNotificationEnabled] = useState(true);
  const [notificationPermission, setNotificationPermission] = useState<AndroidNotificationPermissionState>('unknown');
  const [foregroundServiceActive, setForegroundServiceActive] = useState<boolean | null>(null);
  const [releaseCheck, setReleaseCheck] = useState<ReleaseCheckResult | null>(null);
  const [releaseCheckError, setReleaseCheckError] = useState<string | null>(null);
  const [checkingRelease, setCheckingRelease] = useState(false);
  const [backupMode, setBackupMode] = useState<'export' | 'import' | null>(null);
  const [backupFile, setBackupFile] = useState<{ name: string; text: string } | null>(null);
  const [backupError, setBackupError] = useState<string | null>(null);
  const [backupBusy, setBackupBusy] = useState(false);
  const [backupMessage, setBackupMessage] = useState<string | null>(null);
  const backupDialogRef = useRef<HTMLDialogElement>(null);
  const matches = (...content: string[]) => settingMatches(query, ...content);
  const matchesSetting = (terms: string, ...keys: MessageKey[]) => matches(terms, ...keys.map((key) => t(key)));
  const mark = (text: string) => <SearchMark text={text} query={searchFocused ? query : ''} />;
  const availableLanguages = useMemo(() => supportedLanguages, []);

  useEffect(() => {
    let alive = true;
    getStatus().then((result) => { if (alive) setStatus(result.status); }).catch((error: unknown) => {
      if (alive) setStatusError(error instanceof Error ? error.message : String(error));
    });
    fetchHealth().then((result) => { if (alive) setHealth(result); }).catch(() => undefined);
    let servicePoll: number | undefined;
    if (isAndroidTauriRuntime()) {
      getGatewayServiceStatus().then((gateway) => {
        if (!alive || !gateway?.bind || readApiBasePreference()) return;
        const port = gateway.bind.match(/:(\d+)$/)?.[1] ?? '11435';
        const localBase = `http://127.0.0.1:${port}`;
        saveApiBasePreference(localBase);
        setApiBase(localBase);
        window.dispatchEvent(new Event('jev-api-base-change'));
      }).catch(() => undefined);
      getAndroidKeepaliveNotificationStatus().then((result) => { if (alive) setKeepaliveNotificationEnabled(result.enabled); }).catch(() => undefined);
      getAndroidNotificationPermissionState().then((result) => { if (alive) setNotificationPermission(result); }).catch(() => undefined);
      const pollService = () => getAndroidForegroundServiceActive().then((active) => { if (alive) setForegroundServiceActive(active); }).catch(() => { if (alive) setForegroundServiceActive(null); });
      void pollService();
      servicePoll = window.setInterval(() => { if (!document.hidden) void pollService(); }, 3000);
    }
    const syncProbe = () => setAutoProbe(readAutoProviderProbe());
    const syncStatusBar = () => setShowStatusBar(readShowStatusBar());
    const syncUiScale = () => setUiScale(readUiScale());
    const syncRoutingHud = () => setAutoHideRoutingHud(readRoutingHudAutoHide());
    window.addEventListener(SETTINGS_CHANGE_EVENT, syncProbe);
    window.addEventListener(SETTINGS_CHANGE_EVENT, syncStatusBar);
    window.addEventListener(SETTINGS_CHANGE_EVENT, syncUiScale);
    window.addEventListener(SETTINGS_CHANGE_EVENT, syncRoutingHud);
    return () => { alive = false; window.clearInterval(servicePoll); window.removeEventListener(SETTINGS_CHANGE_EVENT, syncProbe); window.removeEventListener(SETTINGS_CHANGE_EVENT, syncStatusBar); window.removeEventListener(SETTINGS_CHANGE_EVENT, syncUiScale); window.removeEventListener(SETTINGS_CHANGE_EVENT, syncRoutingHud); };
  }, []);

  useEffect(() => {
    const dialog = backupDialogRef.current;
    if (!dialog) return;
    if (backupMode && !dialog.open) dialog.showModal();
    else if (!backupMode && dialog.open) dialog.close();
  }, [backupMode]);

  const checkForUpdates = async () => {
    setCheckingRelease(true);
    setReleaseCheckError(null);
    try {
      setReleaseCheck(await checkLatestRelease(pkg.version));
    } catch (error) {
      setReleaseCheckError(error instanceof Error ? error.message : String(error));
    } finally {
      setCheckingRelease(false);
    }
  };

  const appBackupSettings = () => ({
    theme,
    language,
    autoProviderProbe: autoProbe,
    showStatusBar,
    uiScale,
    autoHideRoutingHud,
    apiBase: readApiBasePreference(),
  });

  const downloadSettingsBackup = async () => {
    setBackupBusy(true);
    setBackupError(null);
    try {
      const gateway = await exportRuntimeConfigJson();
      const backup = createSettingsBackup(appBackupSettings(), gateway);
      await saveJsonFile(`jev-switch-settings-${new Date().toISOString().slice(0, 10)}.json`, JSON.stringify(backup, null, 2));
      setBackupMessage(t('settings.backupExported' as MessageKey));
      setBackupMode(null);
    } catch (error) {
      setBackupError(error instanceof Error ? error.message : String(error));
    } finally {
      setBackupBusy(false);
    }
  };

  const restoreSettingsBackup = async () => {
    if (!backupFile) return;
    setBackupBusy(true);
    setBackupError(null);
    try {
      const backup = parseSettingsBackup(backupFile.text);
      const nextApiBase = backup.app_settings.apiBase === null ? null : normalizeApiBase(backup.app_settings.apiBase);
      await importRuntimeConfigJson(backup.gateway);

      if (nextApiBase === null) clearApiBasePreference();
      else saveApiBasePreference(nextApiBase);
      applyThemePreference(backup.app_settings.theme);
      if (backup.app_settings.language === 'system') { setSystemLang(); setLanguage('system'); }
      else { setLang(backup.app_settings.language); setLanguage(backup.app_settings.language); }
      setTheme(backup.app_settings.theme);
      setApiBase(nextApiBase ?? '');
      setAutoProbe(backup.app_settings.autoProviderProbe);
      setShowStatusBar(backup.app_settings.showStatusBar);
      setAutoHideRoutingHud(backup.app_settings.autoHideRoutingHud);
      setUiScale(writeUiScale(backup.app_settings.uiScale));
      writeAutoProviderProbe(backup.app_settings.autoProviderProbe);
      writeShowStatusBar(backup.app_settings.showStatusBar);
      writeRoutingHudAutoHide(backup.app_settings.autoHideRoutingHud);
      window.dispatchEvent(new Event('jev-api-base-change'));
      window.dispatchEvent(new Event(SETTINGS_CHANGE_EVENT));
      setBackupMessage(t('settings.backupImported' as MessageKey));
      setBackupMode(null);
      setBackupFile(null);
      window.setTimeout(() => window.location.reload(), 250);
    } catch (error) {
      setBackupError(error instanceof Error ? error.message : String(error));
    } finally {
      setBackupBusy(false);
    }
  };

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
    { id: 'personalization', label: t('settings.personalization' as MessageKey), terms: 'personalization dashboard layout home 个性化 首页 布局 仪表盘', content: (
      matchesSetting('personalization dashboard layout home 个性化 首页 布局 仪表盘', 'settings.personalization', 'settings.dashboardLayout', 'settings.dashboardLayoutHint') && <div className="flex flex-wrap items-center justify-between gap-3 text-sm">
        <div className="grid gap-1"><strong>{mark(t('settings.dashboardLayout' as MessageKey))}</strong><span style={{ color: 'var(--text-muted)' }}>{mark(t('settings.dashboardLayoutHint' as MessageKey))}</span></div>
        <a href="#/dashboard?edit=1" className={fieldClass + ' inline-flex items-center'} style={{ ...fieldStyle, color: 'var(--accent)' }}>{t('settings.openDashboardLayout' as MessageKey)}</a>
      </div>
    ) },
    { id: 'appearance', label: t('settings.appearance' as MessageKey), terms: 'appearance theme language status bar ui scale zoom 外观 主题 语言 状态栏 界面缩放 倍率', content: (
      <div className="grid gap-4 sm:grid-cols-2">
        {matchesSetting('theme 主题 appearance 外观', 'settings.theme', 'settings.themeSystem', 'settings.themeLight', 'settings.themeDark') && <label className="grid gap-2 text-sm">
          <span>{mark(t('settings.theme' as MessageKey))}</span>
          <select className={fieldClass} style={fieldStyle} value={theme} onChange={(event) => { const next = event.target.value as ThemePreference; setTheme(next); applyThemePreference(next); }}>
            <option value="system">{t('settings.themeSystem' as MessageKey)}</option>
            <option value="light">{t('settings.themeLight' as MessageKey)}</option>
            <option value="dark">{t('settings.themeDark' as MessageKey)}</option>
          </select>
        </label>}
        {matchesSetting('language locale 语言', 'settings.language') && <label className="grid gap-2 text-sm">
          <span>{mark(t('settings.language' as MessageKey))}</span>
          <select className={fieldClass} style={fieldStyle} value={language} onChange={(event) => {
            const next = event.target.value;
            if (next === 'system') { setSystemLang(); setLanguage('system'); }
            else { setLang(next as typeof lang); setLanguage(next as typeof lang); }
          }}>
            <option value="system">{t('settings.themeSystem' as MessageKey)}</option>
            {availableLanguages.map((item) => <option key={item.code} value={item.code}>{item.label}</option>)}
          </select>
        </label>}
        {matchesSetting('status bar footer 状态栏 底栏', 'settings.showStatusBar', 'settings.showStatusBarHint') && <label className="flex items-start gap-3 text-sm sm:col-span-2">
          <input type="checkbox" checked={showStatusBar} onChange={(event) => { setShowStatusBar(event.target.checked); writeShowStatusBar(event.target.checked); }} className="mt-1 h-4 w-4 accent-[var(--accent)]" />
          <span className="grid gap-1"><strong>{mark(t('settings.showStatusBar' as MessageKey))}</strong><span style={{ color: 'var(--text-muted)' }}>{mark(t('settings.showStatusBarHint' as MessageKey))}</span></span>
        </label>}
        {matchesSetting('ui scale zoom interface scale 界面缩放 倍率', 'settings.uiScale', 'settings.uiScaleHint') && <label className="grid gap-2 text-sm sm:col-span-2">
          <span className="flex items-center justify-between gap-3">
            <span>{mark(t('settings.uiScale' as MessageKey))}</span>
            <span className="inline-flex items-center gap-2">
              <input
                type="number"
                min={UI_SCALE_MIN_PERCENT}
                max={UI_SCALE_MAX_PERCENT}
                step={1}
                value={Math.round(uiScale * 100)}
                onChange={(event) => setUiScale(writeUiScale(Number(event.target.value) / 100))}
                className="h-9 border px-2 text-right tabular"
                style={{ ...fieldStyle, width: '7ch' }}
                aria-label={t('settings.uiScale' as MessageKey)}
              />
              <output className="tabular font-mono" htmlFor="settings-ui-scale">%</output>
            </span>
          </span>
          <input
            id="settings-ui-scale"
            type="range"
            min={UI_SCALE_MIN_PERCENT}
            max={UI_SCALE_MAX_PERCENT}
            step={UI_SCALE_STEP_PERCENT}
            value={Math.round(uiScale * 100)}
            aria-valuetext={`${Math.round(uiScale * 100)}%`}
            onChange={(event) => setUiScale(writeUiScale(Number(event.target.value) / 100))}
            className="w-full accent-[var(--accent)]"
          />
          <span style={{ color: 'var(--text-muted)' }}>{mark(t('settings.uiScaleHint' as MessageKey))}</span>
        </label>}
        {matchesSetting('routing hud toolbar auto hide route canvas 路由 画布 工具栏 自动隐藏', 'settings.autoHideRoutingHud', 'settings.autoHideRoutingHudHint') && <label className="flex items-start gap-3 text-sm sm:col-span-2">
          <input type="checkbox" checked={autoHideRoutingHud} onChange={(event) => { setAutoHideRoutingHud(event.target.checked); writeRoutingHudAutoHide(event.target.checked); }} className="mt-1 h-4 w-4 accent-[var(--accent)]" />
          <span className="grid gap-1"><strong>{mark(t('settings.autoHideRoutingHud' as MessageKey))}</strong><span style={{ color: 'var(--text-muted)' }}>{mark(t('settings.autoHideRoutingHudHint' as MessageKey))}</span></span>
        </label>}
      </div>
    ) },
    { id: 'providers', label: t('settings.probes' as MessageKey), terms: 'provider upstream probe health frequency 提供商 上游 探测 连通性 频率', content: (
      matchesSetting('provider upstream probe health frequency 提供商 上游 探测 连通性 频率', 'settings.autoProbe', 'settings.autoProbeHint') && <label className="flex items-start gap-3 text-sm">
        <input type="checkbox" checked={autoProbe} onChange={(event) => { const value = event.target.checked; setAutoProbe(value); writeAutoProviderProbe(value); }} className="mt-1 h-4 w-4 accent-[var(--accent)]" />
        <span className="grid gap-1"><strong>{mark(t('settings.autoProbe' as MessageKey))}</strong><span style={{ color: 'var(--text-muted)' }}>{mark(t('settings.autoProbeHint' as MessageKey))}</span></span>
      </label>
    ) },
    { id: 'connection', label: t('settings.apiAddress' as MessageKey), terms: 'gateway api url address endpoint connection 网关 API 地址 连接', content: (
      matchesSetting('gateway api url address endpoint connection 网关 API 地址 连接', 'settings.apiAddress', 'settings.apiAddressHint', 'settings.androidApiAddressHint') && <div className="grid gap-2">
        <label className="grid gap-2 text-sm"><span>{mark(t('settings.apiAddress' as MessageKey))}</span><input className={fieldClass} style={fieldStyle} type="url" value={apiBase} onChange={(event) => setApiBase(event.target.value)} placeholder={t('settings.apiAddressPlaceholder' as MessageKey)} /></label>
        <p className="text-xs" style={{ color: 'var(--text-muted)' }}>{mark(t((isAndroidTauriRuntime() ? 'settings.androidApiAddressHint' : 'settings.apiAddressHint') as MessageKey))}</p>
        <div className="flex flex-wrap gap-2"><button type="button" className={fieldClass} style={fieldStyle} onClick={saveApiAddress}>{saved ? t('settings.addressSaved' as MessageKey) : t('settings.saveAddress' as MessageKey)}</button><button type="button" className={fieldClass} style={fieldStyle} onClick={() => { setApiBase(''); clearApiBasePreference(); window.dispatchEvent(new Event('jev-api-base-change')); }}>{t((isAndroidTauriRuntime() ? 'settings.androidResetAddress' : 'settings.resetAddress') as MessageKey)}</button></div>
      </div>
    ) },
    { id: 'settings-backup', label: t('settings.backup' as MessageKey), terms: 'backup restore export import json api key secret config 备份 恢复 导出 导入 密钥 配置', content: (
      matchesSetting('backup restore export import json api key secret config 备份 恢复 导出 导入 密钥 配置', 'settings.backup', 'settings.backupHint', 'settings.exportBackup', 'settings.importBackup') && <div className="grid gap-3">
        <p className="text-sm" style={{ color: 'var(--text-muted)' }}>{mark(t('settings.backupHint' as MessageKey))}</p>
        <div className="flex flex-wrap gap-2">
          <button type="button" className={fieldClass + ' inline-flex items-center gap-2'} style={fieldStyle} onClick={() => { setBackupError(null); setBackupMode('export'); }} disabled={!auth.canManage}><Download size={15} aria-hidden="true" />{t('settings.exportBackup' as MessageKey)}</button>
          <button type="button" className={fieldClass + ' inline-flex items-center gap-2'} style={fieldStyle} onClick={() => { setBackupError(null); setBackupFile(null); setBackupMode('import'); }} disabled={!auth.canManage}><Upload size={15} aria-hidden="true" />{t('settings.importBackup' as MessageKey)}</button>
        </div>
        {backupMessage && <p role="status" className="text-xs" style={{ color: 'var(--success)' }}>{backupMessage}</p>}
        {!auth.canManage && <p className="text-xs" style={{ color: 'var(--text-muted)' }}>{t('settings.adminOnly' as MessageKey)}</p>}
      </div>
    ) },
    ...(isAndroidTauriRuntime() ? [{ id: 'android-debug', label: t('settings.androidDebug' as MessageKey), terms: 'android debug log diagnostics 安卓 调试 日志 诊断', content: (
      matchesSetting('android debug log diagnostics 安卓 调试 日志 诊断', 'settings.androidDebug') && <AndroidDiagnostics />
    ) }] : []),
    ...(isAndroidTauriRuntime() ? [{ id: 'android-keepalive', label: t('settings.androidKeepalive' as MessageKey), terms: 'android foreground service persistent notification background 保活 常驻 通知 后台', content: (
      matchesSetting('android foreground service persistent notification background 保活 常驻 通知 后台', 'settings.androidKeepalive', 'settings.androidKeepaliveToggle', 'settings.androidKeepaliveHint') && <div className="grid gap-3">
        <label className="flex items-start gap-3 text-sm">
          <input type="checkbox" checked={keepaliveNotificationEnabled} onChange={async (event) => {
            const enabled = event.target.checked;
            try {
              if (enabled && notificationPermission !== 'granted') setNotificationPermission(await requestAndroidNotificationPermission());
              const next = await setAndroidKeepaliveNotification(enabled);
              setKeepaliveNotificationEnabled(next.enabled);
            } catch (error) { setStatusError(error instanceof Error ? error.message : String(error)); }
          }} className="mt-1 h-4 w-4 accent-[var(--accent)]" />
          <span className="grid gap-1"><strong>{mark(t('settings.androidKeepaliveToggle' as MessageKey))}</strong><span style={{ color: 'var(--text-muted)' }}>{mark(t('settings.androidKeepaliveHint' as MessageKey))}</span></span>
        </label>
        <p className="text-xs" role="status" style={{ color: notificationPermission === 'granted' ? 'var(--success)' : 'var(--warning)' }}>{t('settings.androidNotificationPermission' as MessageKey, { state: t(`settings.notificationPermission.${notificationPermission}` as MessageKey) })}</p>
        <p className="text-xs" role="status" style={{ color: foregroundServiceActive ? 'var(--success)' : 'var(--text-muted)' }}>
          {t('settings.androidServiceState' as MessageKey, { state: t((foregroundServiceActive === null ? 'settings.androidServiceUnknown' : foregroundServiceActive ? 'settings.androidServiceRunning' : 'settings.androidServiceStopped') as MessageKey) })}
          {' · '}{mark(t('settings.androidServiceStateHint' as MessageKey))}
        </p>
        {notificationPermission !== 'granted' && <button type="button" className={fieldClass + ' inline-flex w-fit items-center gap-2'} style={fieldStyle} onClick={async () => {
          try { setNotificationPermission(await requestAndroidNotificationPermission()); }
          catch (error) { setStatusError(error instanceof Error ? error.message : String(error)); }
        }}>{t('settings.requestNotificationPermission' as MessageKey)}</button>}
      </div>
    ) }] : []),
    { id: 'build-info', label: t('settings.buildInfo' as MessageKey), terms: 'build info version revision api commit 构建 信息 版本 修订 提交', content: (
      matchesSetting('build info version revision api commit 构建 信息 版本 修订 提交', 'settings.buildInfo', 'settings.appVersion', 'settings.backendVersion', 'settings.apiRevision') && <div className="grid gap-3 text-sm sm:grid-cols-2">
        <InfoRow label={t('settings.appVersion' as MessageKey)} value={`v${pkg.version}`} />
        <InfoRow label={t('settings.backendVersion' as MessageKey)} value={health?.version ? `v${health.version}` : '—'} />
        <InfoRow label={t('settings.apiRevision' as MessageKey)} value={health ? String(health.api_revision) : '—'} />
        <InfoRow label={t('settings.uiBuildRevision' as MessageKey)} value={__JEV_UI_BUILD_REVISION__} mono />
        <InfoRow label={t('settings.backendBuildRevision' as MessageKey)} value={health?.build_revision ?? t('settings.notAvailable' as MessageKey)} mono />
        <p className="text-xs sm:col-span-2" style={{ color: 'var(--text-muted)' }}>{t('settings.buildInfoHint' as MessageKey)}</p>
      </div>
    ) },
    { id: 'about', label: t('settings.about' as MessageKey), terms: 'about update github release 关于 更新 发布', content: (
      matchesSetting('about update github release 关于 更新 发布', 'settings.about', 'settings.aboutHint', 'settings.checkUpdates') && <div className="grid gap-4 text-sm">
        <div className="flex flex-wrap items-center gap-2">
          <Info size={16} aria-hidden="true" style={{ color: 'var(--accent)' }} />
          <strong>{t('settings.aboutVersion' as MessageKey, { version: pkg.version })}</strong>
        </div>
        <p style={{ color: 'var(--text-muted)' }}>{t('settings.aboutHint' as MessageKey)}</p>
        <div className="flex flex-wrap gap-2">
          <a className={fieldClass + ' inline-flex items-center gap-2'} style={fieldStyle} href={GITHUB_REPOSITORY_URL} target="_blank" rel="noreferrer"><GitBranch size={15} aria-hidden="true" />{t('settings.githubProject' as MessageKey)}<ExternalLink size={13} aria-hidden="true" /></a>
          <a className={fieldClass + ' inline-flex items-center gap-2'} style={fieldStyle} href={GITHUB_RELEASES_URL} target="_blank" rel="noreferrer"><PackageCheck size={15} aria-hidden="true" />{t('settings.githubReleases' as MessageKey)}<ExternalLink size={13} aria-hidden="true" /></a>
          <button type="button" className={fieldClass + ' inline-flex items-center gap-2'} style={fieldStyle} onClick={() => void checkForUpdates()} disabled={checkingRelease}><RefreshCw size={15} aria-hidden="true" className={checkingRelease ? 'animate-spin' : undefined} />{checkingRelease ? t('settings.checkingUpdates' as MessageKey) : t('settings.checkUpdates' as MessageKey)}</button>
        </div>
        {releaseCheckError && <p role="alert" className="text-xs" style={{ color: 'var(--danger)' }}>{t('settings.updateCheckFailed' as MessageKey, { reason: releaseCheckError })}</p>}
        {releaseCheck && <div className="flex flex-wrap items-center gap-2 text-xs" style={{ color: releaseCheck.updateAvailable ? 'var(--accent)' : 'var(--success)' }}>
          <CheckCircle2 size={15} aria-hidden="true" />
          {releaseCheck.updateAvailable ? <span>{t('settings.updateAvailable' as MessageKey, { version: releaseCheck.latestVersion })} <a href={releaseCheck.releaseUrl} target="_blank" rel="noreferrer">{t('settings.viewRelease' as MessageKey)}</a></span> : <span>{t('settings.upToDate' as MessageKey, { version: releaseCheck.latestVersion })}</span>}
        </div>}
      </div>
    ) },
  ];
  const visibleGroups = groups.filter((group) => matches(group.label, group.terms, ...(groupSearchKeys[group.id] ?? []).map((key) => t(key))));
  const bottomGroups = visibleGroups.filter((group) => group.id === 'build-info' || group.id === 'about');
  const mainGroups = visibleGroups.filter((group) => group.id !== 'build-info' && group.id !== 'about');
  const showInstance = matches('instance mode listen password port 运行模式 监听 密码 端口', ...instanceSearchKeys.map((key) => t(key)));

  return <div className="page-container mx-auto w-full min-w-0">
    <div className="ui-page-title"><h1>{t('settings.title' as MessageKey)}</h1><label className="ui-surface flex min-h-10 w-full max-w-md items-center gap-2 px-3"><Search size={16} aria-hidden="true"/><span className="sr-only">{t('settings.search' as MessageKey)}</span><input autoFocus type="search" value={query} onChange={(event) => setQuery(event.target.value)} onFocus={() => setSearchFocused(true)} onBlur={() => setSearchFocused(false)} placeholder={t('settings.search' as MessageKey)} aria-label={t('settings.search' as MessageKey)} className="min-w-0 flex-1 bg-transparent outline-none"/></label></div>
    <div className="space-y-7">
      {mainGroups.map((group) => <section key={group.id} className="ui-section pb-4"><h2 className="mb-3 font-semibold" style={{ fontSize: 'var(--text-base)' }}>{mark(group.label)}</h2>{group.content}</section>)}
      {showInstance && auth.canManage && <section className="ui-section pb-4"><h2 className="mb-3 font-semibold" style={{ fontSize: 'var(--text-base)' }}>{mark(t('instance.manage' as MessageKey))}</h2>{statusError && <p role="alert" style={{ color: 'var(--danger)' }}>{statusError}</p>}<InstanceSettings status={status} onStatusChange={setStatus} open onOpenChange={() => undefined} searchQuery={query} highlightQuery={searchFocused ? query : ''}/></section>}
      {bottomGroups.map((group) => <section key={group.id} className="ui-section pb-4"><h2 className="mb-3 font-semibold" style={{ fontSize: 'var(--text-base)' }}>{mark(group.label)}</h2>{group.content}</section>)}
      {visibleGroups.length === 0 && !showInstance && <p className="py-8 text-center text-sm" style={{ color: 'var(--text-muted)' }}>{t('settings.noResults' as MessageKey)}</p>}
      {auth.isReadOnly && <p className="text-sm" style={{ color: 'var(--text-muted)' }}>{t('settings.adminOnly' as MessageKey)}</p>}
    </div>
    <dialog ref={backupDialogRef} className="responsive-dialog" aria-labelledby="settings-backup-title" onCancel={(event) => { event.preventDefault(); setBackupMode(null); setBackupFile(null); }} onClose={() => setBackupMode(null)} style={{ ...fieldStyle, color: 'var(--text)', width: 'min(34rem, calc(100vw - 2rem))', padding: 0 }}>
      {backupMode && <div className="grid gap-4 p-5">
        <div className="flex items-center gap-2"><Info size={17} aria-hidden="true" style={{ color: 'var(--warning)' }} /><h2 id="settings-backup-title" className="font-semibold" style={{ fontSize: 'var(--text-lg)' }}>{t('settings.backupWarningTitle' as MessageKey)}</h2></div>
        <p className="text-sm" style={{ color: 'var(--text-muted)' }}>{t('settings.backupWarningBody' as MessageKey)}</p>
        {backupMode === 'import' && <div className="grid gap-2">
          <label className="text-sm" htmlFor="settings-backup-file">{t('settings.backupChooseFile' as MessageKey)}</label>
          <input id="settings-backup-file" type="file" accept="application/json,.json" onChange={async (event) => {
            const file = event.target.files?.[0];
            if (!file) return;
            try { const text = await file.text(); parseSettingsBackup(text); setBackupFile({ name: file.name, text }); setBackupError(null); }
            catch (error) { setBackupFile(null); setBackupError(error instanceof Error ? error.message : String(error)); }
          }} />
          {backupFile && <p className="text-xs" style={{ color: 'var(--text-muted)' }}>{t('settings.backupFileSelected' as MessageKey, { name: backupFile.name })}</p>}
        </div>}
        {backupError && <p role="alert" className="text-xs" style={{ color: 'var(--danger)' }}>{t('settings.backupFailed' as MessageKey, { reason: backupError })}</p>}
        <div className="flex flex-wrap justify-end gap-2">
          <button type="button" className={fieldClass} style={fieldStyle} disabled={backupBusy} onClick={() => { setBackupMode(null); setBackupFile(null); }}>{t('common.cancel' as MessageKey)}</button>
          {backupMode === 'export'
            ? <button type="button" className={fieldClass} style={{ ...fieldStyle, borderColor: 'var(--accent)', background: 'var(--accent)', color: '#fff' }} disabled={backupBusy} onClick={() => void downloadSettingsBackup()}>{backupBusy ? t('common.saving' as MessageKey) : t('settings.backupConfirmExport' as MessageKey)}</button>
            : <button type="button" className={fieldClass} style={{ ...fieldStyle, borderColor: 'var(--accent)', background: 'var(--accent)', color: '#fff' }} disabled={backupBusy || !backupFile} onClick={() => void restoreSettingsBackup()}>{backupBusy ? t('common.saving' as MessageKey) : t('settings.backupApplyImport' as MessageKey)}</button>}
        </div>
      </div>}
    </dialog>
  </div>;
}

function InfoRow({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return <div className="grid gap-1"><span className="text-xs" style={{ color: 'var(--text-muted)' }}>{label}</span><span className={mono ? 'break-all font-mono text-xs' : 'tabular'}>{value}</span></div>;
}
