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
      className="inline-flex items-center rounded-ctl border border-border bg-panel px-2 py-0.5 font-mono text-[10px] font-semibold uppercase tracking-widest text-inkMuted transition-colors hover:border-primaryBright hover:bg-soft hover:text-primary"
    >
      <span>{lang === 'en' ? 'EN' : '中'}</span>
    </button>
  );
}
