import { Globe } from 'lucide-react';
import { useI18n, type MessageKey } from '../../i18n';
import { supportedLanguages } from '../../i18n/languages';

/** Locale selector generated from the registry of complete translations. */
export function LangToggle() {
  const { lang, setLang, t } = useI18n();
  return (
    <label
      className="inline-flex h-8 items-center gap-1.5 px-2 transition-colors"
      style={{
        border: '1px solid var(--border)',
        borderRadius: 'var(--radius)',
        background: 'var(--surface-hover)',
        color: 'var(--text-muted)',
        fontSize: 'var(--text-sm)',
        fontWeight: 500,
      }}
    >
      <Globe size={14} strokeWidth={2} aria-hidden="true" />
      <span className="sr-only">{t('locale.language' as MessageKey)}</span>
      <select
        aria-label={t('locale.language' as MessageKey)}
        value={lang}
        onChange={(event) => setLang(event.currentTarget.value as typeof lang)}
        className="cursor-pointer bg-transparent outline-none"
        style={{ color: 'inherit', font: 'inherit' }}
      >
        {supportedLanguages.map((language) => (
          <option key={language.code} value={language.code}>
            {language.label}
          </option>
        ))}
      </select>
    </label>
  );
}
