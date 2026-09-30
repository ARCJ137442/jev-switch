export const API_BASE_STORAGE_KEY = 'jev_api_base';

export function normalizeApiBase(value: string): string {
  const trimmed = value.trim();
  const parsed = new URL(trimmed);
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') throw new Error('API address must use HTTP or HTTPS');
  if (parsed.username || parsed.password || parsed.search || parsed.hash) throw new Error('API address cannot contain credentials, query, or fragment');
  return trimmed.replace(/\/+$/, '');
}

export function readApiBasePreference(storage?: Pick<Storage, 'getItem'>): string | null {
  try {
    const value = (storage ?? (typeof localStorage === 'undefined' ? undefined : localStorage))?.getItem(API_BASE_STORAGE_KEY);
    return value ? normalizeApiBase(value) : null;
  } catch {
    return null;
  }
}

export function saveApiBasePreference(value: string, storage?: Pick<Storage, 'setItem'>): string {
  const normalized = normalizeApiBase(value);
  const target = storage ?? (typeof localStorage === 'undefined' ? undefined : localStorage);
  if (!target) throw new Error('Persistent storage is unavailable');
  target.setItem(API_BASE_STORAGE_KEY, normalized);
  return normalized;
}

export function clearApiBasePreference(storage?: Pick<Storage, 'removeItem'>): void {
  (storage ?? (typeof localStorage === 'undefined' ? undefined : localStorage))?.removeItem(API_BASE_STORAGE_KEY);
}

/** A deployed console follows its serving origin unless the user configured an API endpoint. */
export function resolveApiBase(pageUrl: string | undefined, isDevelopment: boolean, override: unknown, developmentBase: string): string {
  if (typeof override === 'string' && override.length > 0) return override;
  if (pageUrl && !isDevelopment) {
    const protocol = new URL(pageUrl).protocol;
    if (protocol === 'http:' || protocol === 'https:') return '';
  }
  return developmentBase;
}
