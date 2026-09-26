import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { getLang, setLang as coreSetLang, t, type Lang, type MessageKey } from './core';
import { supportedLanguages } from './languages';

export { t, initLang } from './core';
export type { Lang, MessageKey };

interface I18nContextValue {
  lang: Lang;
  setLang: (next: Lang) => void;
  /** Legacy convenience API; cycles through currently registered languages. */
  toggleLang: () => void;
  t: (key: MessageKey, vars?: Record<string, string | number>) => string;
}

const I18nContext = createContext<I18nContextValue | null>(null);

/** Bridge the module-level translator to React updates and expose locale selection. */
export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(() => getLang());

  const setLang = useCallback((next: Lang) => {
    coreSetLang(next);
    setLangState(getLang());
  }, []);

  const toggleLang = useCallback(() => {
    const currentIndex = supportedLanguages.findIndex((item) => item.code === getLang());
    const next = supportedLanguages[(currentIndex + 1) % supportedLanguages.length];
    if (next) setLang(next.code);
  }, [setLang]);

  const value = useMemo<I18nContextValue>(
    () => ({ lang, setLang, toggleLang, t: (key, vars) => t(key, vars) }),
    [lang, setLang, toggleLang],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    return { lang: getLang(), setLang: coreSetLang, toggleLang: () => {}, t };
  }
  return ctx;
}
