import { useState } from 'react';
import { probeProvider, type ProbeResponse } from '../../api/admin';
import { useI18n } from '../../i18n';

interface Props {
  providerId: string;
  onResult: (result: ProbeResponse) => void;
}

/**
 * Probe 按钮（design/01 §6.1）：POST /v1/admin/providers/{id}/probe；
 * loading 态按钮文案，结果回传卡片 footer。en='Probe'（CDP PROBE 前缀断言）。
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
      onResult({ ok: false, latency_ms: 0, status: null, error: (e as Error).message });
    } finally {
      setLoading(false);
    }
  };

  return (
    <button
      type="button"
      onClick={() => void onClick()}
      disabled={loading}
      className="h-8 border border-border bg-panel px-2.5 font-mono text-xs text-ink hover:border-primaryBright hover:bg-soft disabled:cursor-not-allowed disabled:opacity-60"
    >
      {loading ? t('probe.loading') : t('probe.btn')}
    </button>
  );
}
