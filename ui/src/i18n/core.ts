import { en, type MessageKey } from './en';
import { zh } from './zh';

/**
 * i18n 核心（零依赖 — 模块级语言状态，非 React 调用方如 api/admin.ts 直接用 t()）。
 * 默认 en（CDP 断言依赖英文字面）；`localStorage['jev_lang']` 持久；
 * 切换同步 `document.documentElement.lang`。
 */
export type Lang = 'en' | 'zh';
export type { MessageKey };

const DICTS: Record<Lang, Record<MessageKey, string>> = { en, zh };

let lang: Lang = 'en';

export function getLang(): Lang {
  return lang;
}

/** 设置语言：写模块状态 + localStorage + <html lang>（React 层另行触发重渲染） */
export function setLang(next: Lang): void {
  lang = next;
  try {
    localStorage.setItem('jev_lang', next);
  } catch {
    /* 隐私模式 → 仅本会话 */
  }
  document.documentElement.lang = next;
}

/** 首帧前初始化（main.tsx 调用）：jev_lang → 默认 en */
export function initLang(): void {
  let stored: string | null = null;
  try {
    stored = localStorage.getItem('jev_lang');
  } catch {
    /* ignore */
  }
  lang = stored === 'zh' ? 'zh' : 'en';
  document.documentElement.lang = lang;
}

/** 取译文并做 {name} 插值；缺键回退英文，再缺回退键名 */
export function t(key: MessageKey, vars?: Record<string, string | number>): string {
  const raw = DICTS[lang][key] ?? en[key] ?? key;
  if (!vars) return raw;
  return raw.replace(/\{(\w+)\}/g, (m, name: string) =>
    Object.prototype.hasOwnProperty.call(vars, name) ? String(vars[name]) : m,
  );
}
