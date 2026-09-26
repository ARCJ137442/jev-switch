import { en } from './en';
import { zh } from './zh';

export const LANG_STORAGE_KEY = 'jev_lang';
export const DEFAULT_LANG = 'en';

export const languageRegistry = {
  en: { label: 'English', htmlLang: 'en', dictionary: en },
  zh: { label: '简体中文', htmlLang: 'zh-CN', dictionary: zh },
} as const;

export type Lang = keyof typeof languageRegistry;
export type MessageKey = keyof typeof en;
export const languageDictionaries = Object.fromEntries(
  Object.entries(languageRegistry).map(([code, entry]) => [code, entry.dictionary]),
) as Record<Lang, Record<MessageKey, string>>;
export const supportedLanguages = Object.entries(languageRegistry).map(([code, entry]) => ({
  code: code as Lang,
  label: entry.label,
}));

export function isLang(value: unknown): value is Lang {
  return typeof value === 'string' && Object.prototype.hasOwnProperty.call(languageRegistry, value);
}

export function resolveLang(value: unknown): Lang {
  return isLang(value) ? value : 'en';
}

export function matchRegisteredLanguage(
  browserLanguage: string,
  registry: Record<string, { htmlLang: string }>,
): string | null {
  const candidate = browserLanguage.toLowerCase().replace(/_/g, '-');
  const registered = Object.entries(registry) as [string, { htmlLang: string }][];
  const exactMatch = registered.find(
    ([code, entry]) =>
      code.toLowerCase() === candidate || entry.htmlLang.toLowerCase() === candidate,
  );
  if (exactMatch) return exactMatch[0];

  const primaryLanguage = candidate.split('-')[0];
  const prefixMatch = registered.find(([code, entry]) => {
    const registeredPrimary = entry.htmlLang.toLowerCase().split('-')[0];
    return code.toLowerCase() === primaryLanguage || registeredPrimary === primaryLanguage;
  });
  return prefixMatch?.[0] ?? null;
}

export function detectBrowserLang(browserLanguage?: string): Lang {
  const detected =
    browserLanguage ??
    (typeof navigator === 'undefined' ? '' : navigator.language || navigator.languages?.[0] || '');
  const matched = matchRegisteredLanguage(detected, languageRegistry);
  return isLang(matched) ? matched : DEFAULT_LANG;
}
