import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { getLang, setLang as coreSetLang, t, type Lang, type MessageKey } from './core';

export { t, initLang } from './core';
export type { Lang, MessageKey };

interface I18nContextValue {
  lang: Lang;
  /** 切换语言（en ⇄ zh）并持久化；触发整树重渲染 */
  toggleLang: () => void;
  /** 按键取当前语言译文 */
  t: (key: MessageKey, vars?: Record<string, string | number>) => string;
}

const I18nContext = createContext<I18nContextValue | null>(null);

/**
 * I18nProvider — 包住 App（main 已 initLang，此处只补 React 重渲染桥）。
 * t 从模块核心读取，lang 变化时通过 useState 版本号强制子树更新。
 */
export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(() => getLang());

  const toggleLang = useCallback(() => {
    const next = getLang() === 'en' ? 'zh' : 'en';
    coreSetLang(next);
    setLangState(next);
  }, []);

  const value = useMemo<I18nContextValue>(
    () => ({
      lang,
      toggleLang,
      t: (key, vars) => t(key, vars),
    }),
    [lang, toggleLang],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    // 未包 Provider（测试/隔离渲染）→ 降级为模块核心直读
    return { lang: getLang(), toggleLang: () => {}, t };
  }
  return ctx;
}
