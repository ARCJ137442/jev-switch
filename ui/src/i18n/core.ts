import { en } from './en';
import {
  detectBrowserLang,
  LANG_STORAGE_KEY,
  languageDictionaries,
  languageRegistry,
  isLang,
  resolveLang,
  type Lang,
  type MessageKey,
} from './languages';

export type { Lang, MessageKey };

let lang: Lang = 'en';
const SYSTEM_LANGUAGE = 'system';

export function getLang(): Lang {
  return lang;
}

/** Set the active locale, persist when possible, and keep document metadata aligned. */
export function setLang(next: unknown): void {
  lang = resolveLang(next);
  try {
    localStorage.setItem(LANG_STORAGE_KEY, lang);
  } catch {
    /* Private browsing or denied storage: use this locale for the current session. */
  }
  if (typeof document !== 'undefined') {
    document.documentElement.lang = languageRegistry[lang].htmlLang;
  }
}

export function setSystemLang(): void {
  lang = detectBrowserLang();
  try {
    localStorage.setItem(LANG_STORAGE_KEY, SYSTEM_LANGUAGE);
  } catch {
    /* Keep the detected locale for this session if storage is unavailable. */
  }
  if (typeof document !== 'undefined') document.documentElement.lang = languageRegistry[lang].htmlLang;
}

export function getLangPreference(): Lang | typeof SYSTEM_LANGUAGE {
  try {
    const stored = localStorage.getItem(LANG_STORAGE_KEY);
    if (stored === SYSTEM_LANGUAGE) return SYSTEM_LANGUAGE;
    if (isLang(stored)) return stored;
  } catch {
    /* Unavailable storage uses system detection. */
  }
  return SYSTEM_LANGUAGE;
}

/** Resolve a valid saved locale first, then a supported browser locale, then English. */
export function initLang(): void {
  let stored: string | null = null;
  try {
    stored = localStorage.getItem(LANG_STORAGE_KEY);
  } catch {
    /* Ignore unavailable storage and detect from the browser. */
  }
  lang = stored !== null && stored !== SYSTEM_LANGUAGE ? resolveLang(stored) : detectBrowserLang();
  if (typeof document !== 'undefined') {
    document.documentElement.lang = languageRegistry[lang].htmlLang;
  }
}

/** Translate, falling back to English for missing entries and then to the key itself. */
export function t(key: MessageKey, vars?: Record<string, string | number>): string {
  const raw = languageDictionaries[lang][key] ?? en[key] ?? key;
  if (!vars) return raw;
  return raw.replace(/\{(\w+)\}/g, (m, name: string) =>
    Object.prototype.hasOwnProperty.call(vars, name) ? String(vars[name]) : m,
  );
}
