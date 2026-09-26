import { useState } from 'react';
import { probeProvider, type ProbeResponse } from '../../api/admin';
import { useI18n, type MessageKey } from '../../i18n';

interface Props {
  providerId: string;
  onResult: (result: ProbeResponse) => void;
}

/**
 * Probe 按钮（v2 设计系统）：POST /v1/admin/providers/{id}/probe；
 * loading 态换文案，结果回传卡片 footer。en='Probe'（CDP PROBE 前缀断言）。
 */
export function ProbeButton({ providerId, onResult }: Props) {
  const { t } = useI18n();
  const [loading, setLoading] = useState(false);

  const onClick = async () => {
    setLoading(true);
    try {
      const result = await probeProvider(providerId);
      onResult(result);
    } catch (e) {
      onResult({ ok: false, latency_ms: 0, status: 0, error: (e as Error).message });
    } finally {
      setLoading(false);
    }
  };

  return (
    <button
      type="button"
      onClick={() => void onClick()}
      disabled={loading}
      title={t('providers.probeTip' as MessageKey)}
      className="transition-colors disabled:cursor-not-allowed disabled:opacity-60"
      style={{
        fontSize: 'var(--text-sm)',
        background: 'var(--surface-hover)',
        border: '1px solid var(--border)',
        borderRadius: 'var(--radius)',
        color: 'var(--text)',
        padding: '0.35rem 0.7rem',
      }}
    >
      {loading ? t('probe.loading') : t('probe.btn')}
    </button>
  );
}
