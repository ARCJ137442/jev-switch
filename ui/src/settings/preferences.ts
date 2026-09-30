export const AUTO_PROVIDER_PROBE_KEY = 'jev_auto_provider_probe';
export const SHOW_STATUS_BAR_KEY = 'jev_show_status_bar';
export const SETTINGS_CHANGE_EVENT = 'jev-settings-change';

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

export function toggleFromStorage(event: Pick<StorageEvent, 'key' | 'newValue'>): boolean | null {
  if (event.key !== AUTO_PROVIDER_PROBE_KEY) return null;
  return event.newValue !== 'false';
}
