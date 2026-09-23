import { useI18n } from '../../i18n';

/**
 * 语言切换钮（块 3 建，块 5 抽共享组件）——
 * Shell 顶栏第二行 + 首页操作区两处同源渲染；
 * I18nProvider 状态驱动，多实例自动同步；显示当前语言，点击对切。
 */
export function LangToggle() {
  const { lang, toggleLang, t } = useI18n();
  return (
    <button
      type="button"
      onClick={toggleLang}
      aria-label={lang === 'en' ? t('shell.langToZh') : t('shell.langToEn')}
      className="inline-flex h-8 min-w-8 items-center justify-center px-2 transition-colors"
      style={{
        border: '1px solid var(--border)',
        borderRadius: 'var(--radius)',
        background: 'var(--surface-hover)',
        color: 'var(--text-muted)',
        fontSize: 'var(--text-sm)',
        fontWeight: 500,
      }}
      title={lang === 'en' ? t('shell.langToZh') : t('shell.langToEn')}
    >
      <span>{lang === 'en' ? 'EN' : '中'}</span>
    </button>
  );
}
