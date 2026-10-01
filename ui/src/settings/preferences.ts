export const AUTO_PROVIDER_PROBE_KEY = 'jev_auto_provider_probe';
export const SHOW_STATUS_BAR_KEY = 'jev_show_status_bar';
export const UI_SCALE_KEY = 'jev_ui_scale';
export const ROUTING_HUD_AUTO_HIDE_KEY = 'jev_routing_hud_auto_hide';
export const SETTINGS_CHANGE_EVENT = 'jev-settings-change';

export const UI_SCALE_MIN_PERCENT = 80;
export const UI_SCALE_MAX_PERCENT = 125;
export const UI_SCALE_STEP_PERCENT = 5;

function normalizeUiScale(value: number): number {
  if (!Number.isFinite(value)) return 1;
  return Math.min(UI_SCALE_MAX_PERCENT / 100, Math.max(UI_SCALE_MIN_PERCENT / 100, value));
}

export function readUiScale(storage?: Pick<Storage, 'getItem'>): number {
  try {
    const value = (storage ?? (typeof localStorage === 'undefined' ? undefined : localStorage))?.getItem(UI_SCALE_KEY);
    return value === null || value === undefined ? 1 : normalizeUiScale(Number(value));
  } catch {
    return 1;
  }
}

export function applyUiScale(scale: number): number {
  const normalized = normalizeUiScale(scale);
  if (typeof document !== 'undefined') {
    document.documentElement.style.setProperty('zoom', String(normalized));
  }
  return normalized;
}

export function writeUiScale(scale: number, target?: Pick<Storage, 'setItem'>): number {
  const normalized = normalizeUiScale(scale);
  const storage = target ?? (typeof localStorage === 'undefined' ? undefined : localStorage);
  try {
    storage?.setItem(UI_SCALE_KEY, String(normalized));
  } catch {
    // Keep the in-memory preference for this page session if storage is unavailable.
  }
  applyUiScale(normalized);
  if (typeof window !== 'undefined') window.dispatchEvent(new Event(SETTINGS_CHANGE_EVENT));
  return normalized;
}

export function readAutoProviderProbe(storage?: Pick<Storage, 'getItem'>): boolean {
  try {
    const value = (storage ?? (typeof localStorage === 'undefined' ? undefined : localStorage))?.getItem(AUTO_PROVIDER_PROBE_KEY);
    return value !== 'false';
  } catch {
    return true;
  }
}

export function writeAutoProviderProbe(enabled: boolean, target?: Pick<Storage, 'setItem'>): void {
  const storage = target ?? (typeof localStorage === 'undefined' ? undefined : localStorage);
  try {
    storage?.setItem(AUTO_PROVIDER_PROBE_KEY, String(enabled));
  } catch {
    // Keep the in-memory preference for this page session if storage is unavailable.
  }
  if (typeof window !== 'undefined') window.dispatchEvent(new Event(SETTINGS_CHANGE_EVENT));
}

export function readShowStatusBar(storage?: Pick<Storage, 'getItem'>): boolean {
  try {
    const value = (storage ?? (typeof localStorage === 'undefined' ? undefined : localStorage))?.getItem(SHOW_STATUS_BAR_KEY);
    return value !== 'false';
  } catch {
    return true;
  }
}

export function writeShowStatusBar(enabled: boolean, target?: Pick<Storage, 'setItem'>): void {
  const storage = target ?? (typeof localStorage === 'undefined' ? undefined : localStorage);
  try {
    storage?.setItem(SHOW_STATUS_BAR_KEY, String(enabled));
  } catch {
    // The preference still applies to the current shell session.
  }
  if (typeof window !== 'undefined') window.dispatchEvent(new Event(SETTINGS_CHANGE_EVENT));
}

export function readRoutingHudAutoHide(storage?: Pick<Storage, 'getItem'>): boolean {
  try {
    const value = (storage ?? (typeof localStorage === 'undefined' ? undefined : localStorage))?.getItem(ROUTING_HUD_AUTO_HIDE_KEY);
    return value !== 'false';
  } catch {
    return true;
  }
}

export function writeRoutingHudAutoHide(enabled: boolean, target?: Pick<Storage, 'setItem'>): void {
  const storage = target ?? (typeof localStorage === 'undefined' ? undefined : localStorage);
  try {
    storage?.setItem(ROUTING_HUD_AUTO_HIDE_KEY, String(enabled));
  } catch {
    // Keep the current setting active if browser storage is unavailable.
  }
  if (typeof window !== 'undefined') window.dispatchEvent(new Event(SETTINGS_CHANGE_EVENT));
}

export function toggleFromStorage(event: Pick<StorageEvent, 'key' | 'newValue'>): boolean | null {
  if (event.key !== AUTO_PROVIDER_PROBE_KEY) return null;
  return event.newValue !== 'false';
}
