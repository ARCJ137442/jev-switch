import { useI18n, type MessageKey } from '../i18n';
import { AccessDashboard } from '../components/access/AccessDashboard';

export function StatisticsPage() {
  const { t } = useI18n();
  return <div className="page-container mx-auto w-full min-w-0">
    <div className="ui-page-title"><h1>{t('stats.title' as MessageKey)}</h1><p>{t('stats.subtitle' as MessageKey)}</p></div>
    <AccessDashboard />
  </div>;
}
