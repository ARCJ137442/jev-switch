import { EndpointPanel } from '../components/routing/EndpointPanel';
import { useI18n } from '../i18n';

export function EndpointsPage() {
  const { t } = useI18n();
  return (
    <div className="w-full min-w-0 px-4 py-4 sm:px-6 lg:px-8">
      <h1 className="mb-5 font-semibold" style={{ fontSize: 'var(--text-2xl)' }}>{t('entry.tab')}</h1>
      <EndpointPanel onChanged={() => undefined} />
    </div>
  );
}
