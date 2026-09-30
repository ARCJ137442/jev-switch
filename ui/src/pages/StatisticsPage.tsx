import { useI18n, type MessageKey } from '../i18n';
import { AccessDashboard } from '../components/access/AccessDashboard';

export function StatisticsPage() {
  const { t } = useI18n();
  return <div className="page-container mx-auto w-full min-w-0">
    <header className="mb-5"><h1 className="font-semibold" style={{ fontSize: 'var(--text-2xl)' }}>{t('stats.title' as MessageKey)}</h1><p className="mt-1 text-sm" style={{ color: 'var(--text-muted)' }}>{t('stats.subtitle' as MessageKey)}</p></header>
    <AccessDashboard />
  </div>;
}
