import type { Lang } from '../i18n';
import type { ThemePreference } from '../theme';
import type { RuntimeConfigBackup } from '../api/configFile';

export const SETTINGS_BACKUP_FORMAT = 'jev-switch-settings';
export const SETTINGS_BACKUP_SCHEMA_VERSION = 1;

export interface AppSettingsBackup {
  theme: ThemePreference;
  language: Lang | 'system';
  autoProviderProbe: boolean;
  showStatusBar: boolean;
  uiScale: number;
  autoHideRoutingHud: boolean;
  apiBase: string | null;
}

export interface SettingsBackup {
  format: typeof SETTINGS_BACKUP_FORMAT;
  schema_version: typeof SETTINGS_BACKUP_SCHEMA_VERSION;
  exported_at: string;
  app_settings: AppSettingsBackup;
  gateway: RuntimeConfigBackup;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function parseAppSettings(value: unknown): AppSettingsBackup {
  if (!isObject(value)) throw new Error('app_settings must be an object');
  const { theme, language, autoProviderProbe, showStatusBar, uiScale, autoHideRoutingHud, apiBase } = value;
  if (theme !== 'system' && theme !== 'light' && theme !== 'dark') throw new Error('invalid theme');
  if (language !== 'system' && language !== 'en' && language !== 'zh') throw new Error('invalid language');
  if (typeof autoProviderProbe !== 'boolean' || typeof showStatusBar !== 'boolean' || typeof autoHideRoutingHud !== 'boolean') {
    throw new Error('invalid boolean application setting');
  }
  if (typeof uiScale !== 'number' || !Number.isFinite(uiScale) || uiScale < 0.1 || uiScale > 2) throw new Error('invalid UI scale');
  if (apiBase !== null && typeof apiBase !== 'string') throw new Error('invalid API base');
  return { theme, language, autoProviderProbe, showStatusBar, uiScale, autoHideRoutingHud, apiBase };
}

function parseGateway(value: unknown): RuntimeConfigBackup {
  if (!isObject(value) || value.schema_version !== 1 || !isObject(value.providers) || !Array.isArray(value.routes)) {
    throw new Error('invalid gateway configuration');
  }
  for (const [id, provider] of Object.entries(value.providers)) {
    if (!id.trim() || !isObject(provider) || typeof provider.kind !== 'string' || typeof provider.base !== 'string' || typeof provider.enabled !== 'boolean' || !Array.isArray(provider.models) || !provider.models.every((model) => typeof model === 'string')) {
      throw new Error(`invalid provider entry: ${id || '(empty)'}`);
    }
    if (provider.api_key !== undefined && provider.api_key !== null && typeof provider.api_key !== 'string') throw new Error(`invalid API key field: ${id}`);
  }
  return value as unknown as RuntimeConfigBackup;
}

export function parseSettingsBackup(text: string): SettingsBackup {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error('invalid JSON');
  }
  if (!isObject(parsed) || parsed.format !== SETTINGS_BACKUP_FORMAT || parsed.schema_version !== SETTINGS_BACKUP_SCHEMA_VERSION || typeof parsed.exported_at !== 'string') {
    throw new Error('unsupported Jev-Switch settings backup');
  }
  return {
    format: SETTINGS_BACKUP_FORMAT,
    schema_version: SETTINGS_BACKUP_SCHEMA_VERSION,
    exported_at: parsed.exported_at,
    app_settings: parseAppSettings(parsed.app_settings),
    gateway: parseGateway(parsed.gateway),
  };
}

export function createSettingsBackup(appSettings: AppSettingsBackup, gateway: RuntimeConfigBackup, exportedAt = new Date().toISOString()): SettingsBackup {
  return {
    format: SETTINGS_BACKUP_FORMAT,
    schema_version: SETTINGS_BACKUP_SCHEMA_VERSION,
    exported_at: exportedAt,
    app_settings: appSettings,
    gateway,
  };
}
