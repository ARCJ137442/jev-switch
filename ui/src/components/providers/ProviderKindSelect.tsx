import type { ChangeEvent } from 'react';
import { useI18n, type MessageKey } from '../../i18n';
import { PROVIDER_KINDS } from '../../api/providerKinds';

interface Props {
  value: string;
  onChange: (value: string) => void;
  className?: string;
  style?: React.CSSProperties;
  id?: string;
}

export function ProviderKindSelect({ value, onChange, className, style, id }: Props) {
  const { t } = useI18n();
  const currentIsKnown = PROVIDER_KINDS.some((kind) => kind.value === value);
  const selected = PROVIDER_KINDS.find((kind) => kind.value === value);

  const handleChange = (event: ChangeEvent<HTMLSelectElement>) => onChange(event.target.value);

  return (
    <>
      <select id={id} value={value} onChange={handleChange} className={className} style={style}>
        <option value="">{t('providers.kindPlaceholder' as MessageKey)}</option>
        {!currentIsKnown && value && (
          <option value={value} disabled>{t('providers.kindUnsupported' as MessageKey, { kind: value })}</option>
        )}
        {PROVIDER_KINDS.map((kind) => (
          <option key={kind.value} value={kind.value}>{t(kind.labelKey as MessageKey)}</option>
        ))}
      </select>
      <span style={{ color: 'var(--text-subtle)', fontSize: 'var(--text-xs)' }}>
        {selected
          ? t(selected.descriptionKey as MessageKey)
          : value
            ? t('providers.kindUnsupported' as MessageKey, { kind: value })
            : t('providers.kindHelp' as MessageKey)}
      </span>
    </>
  );
}
