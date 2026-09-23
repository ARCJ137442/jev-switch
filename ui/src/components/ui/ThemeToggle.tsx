import { useEffect, useState } from 'react';
import { useI18n } from '../../i18n';

function readTheme(): 'light' | 'dark' {
  return document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light';
}

/**
 * 主题切换钮（块 2 建，块 5 抽共享组件）——
 * Shell 顶栏第二行 + 首页操作区两处同源渲染；
 * 多实例经 `jev-theme-change` 事件互相同步（main.tsx 首帧已写 data-theme）。
 */
export function ThemeToggle() {
  const { t } = useI18n();
  const [theme, setTheme] = useState<'light' | 'dark'>(readTheme);

  useEffect(() => {
    const sync = () => setTheme(readTheme());
    window.addEventListener('jev-theme-change', sync);
    return () => window.removeEventListener('jev-theme-change', sync);
  }, []);

  const toggle = () => {
    const next = theme === 'dark' ? 'light' : 'dark';
    document.documentElement.dataset.theme = next;
    try {
      localStorage.setItem('jev_theme', next);
    } catch {
      /* 隐私模式 → 仅本会话生效 */
    }
    window.dispatchEvent(new Event('jev-theme-change'));
  };

  return (
    <button
      type="button"
      onClick={toggle}
      aria-label={theme === 'dark' ? t('shell.themeToLight') : t('shell.themeToDark')}
      aria-pressed={theme === 'dark'}
      className="inline-flex items-center gap-1 rounded-ctl border border-border bg-panel px-2 py-0.5 font-mono text-[10px] uppercase tracking-widest text-inkMuted transition-colors hover:border-primaryBright hover:bg-soft hover:text-primary"
    >
      <span aria-hidden>{theme === 'dark' ? '☾' : '☀'}</span>
      <span>{theme === 'dark' ? 'DARK' : 'LIGHT'}</span>
    </button>
  );
}
