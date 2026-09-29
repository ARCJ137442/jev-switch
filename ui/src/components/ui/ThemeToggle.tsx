import { useEffect, useState } from 'react';
import { Sun, Moon, SunMoon } from 'lucide-react';
import { useI18n } from '../../i18n';
import { applyThemePreference, readThemePreference, type ThemePreference } from '../../theme';

/**
 * 主题切换钮（块 2 建，块 5 抽共享组件）——
 * Shell 顶栏第二行 + 首页操作区两处同源渲染；
 * 多实例经 `jev-theme-change` 事件互相同步（main.tsx 首帧已写 data-theme）。
 */
export function ThemeToggle() {
  const { t } = useI18n();
  const [preference, setPreference] = useState<ThemePreference>(readThemePreference);

  useEffect(() => {
    const sync = () => setPreference(readThemePreference());
    window.addEventListener('jev-theme-change', sync);
    return () => window.removeEventListener('jev-theme-change', sync);
  }, []);

  const toggle = () => {
    const next: ThemePreference = preference === 'light' ? 'dark' : preference === 'dark' ? 'system' : 'light';
    setPreference(next);
    applyThemePreference(next);
  };

  const Icon = preference === 'dark' ? Moon : preference === 'light' ? Sun : SunMoon;
  const label = preference === 'dark' ? t('shell.themeModeDark') : preference === 'light' ? t('shell.themeModeLight') : t('shell.themeModeSystem');

  return (
    <button
      type="button"
      onClick={toggle}
      aria-label={label}
      aria-pressed={preference === 'dark'}
      className="inline-flex h-8 w-8 items-center justify-center transition-colors"
      style={{
        border: '1px solid var(--border)',
        borderRadius: 'var(--radius)',
        background: 'var(--surface-hover)',
        color: 'var(--text-muted)',
        fontSize: 'var(--text-sm)',
      }}
      title={label}
    >
      <Icon size={16} strokeWidth={2} />
    </button>
  );
}
