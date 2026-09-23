import { useState } from 'react';
import { probeProvider, type ProbeResponse } from '../../api/admin';

interface Props {
  providerId: string;
  onResult: (result: ProbeResponse) => void;
}

/**
 * Probe 按钮（design/01 §6.1）：POST /v1/admin/providers/{id}/probe；
 * loading 态按钮文案 Probing…，结果回传卡片 footer。
 */
export function ProbeButton({ providerId, onResult }: Props) {
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
      className="border border-border bg-panel px-2.5 py-1 font-mono text-[11px] text-ink hover:border-ink disabled:cursor-not-allowed disabled:opacity-60"
    >
      {loading ? 'Probing…' : 'Probe'}
    </button>
  );
}
