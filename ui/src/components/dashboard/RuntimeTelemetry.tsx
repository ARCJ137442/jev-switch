import { Pause, Play, RefreshCw, Activity, Cpu, HardDrive, ArrowDownToLine, ArrowUpFromLine } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getTelemetry, type RuntimeTelemetry as RuntimeTelemetrySnapshot } from '../../api/admin';
import { useI18n } from '../../i18n';
import { buildTrafficWindow, TRAFFIC_WINDOW_MS } from '../../pages/telemetryWindow';
import './runtime-telemetry.css';

const REFRESH_OPTIONS = [1000, 5000] as const;

function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || !Number.isFinite(bytes)) return '—';
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = -1;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${units[unit]}`;
}

function formatRate(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

function formatPercent(value: number | null | undefined): string {
  return value == null || !Number.isFinite(value) ? '—' : `${value.toFixed(value >= 10 ? 0 : 1)}%`;
}

function formatLatency(value: number | null | undefined): string {
  return value == null || !Number.isFinite(value) ? '—' : `${value.toFixed(value >= 100 ? 0 : 1)} ms`;
}

function successRate(data: RuntimeTelemetrySnapshot): number | null {
  if (data.total_requests === 0) return null;
  return (data.success_requests / data.total_requests) * 100;
}

function points(values: number[], width: number, height: number, max: number): string {
  if (values.length < 2) return '';
  return values.map((value, index) => {
    const x = (index / (values.length - 1)) * width;
    const y = height - (Math.min(value, max) / max) * height;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(' ');
}

function Metric({ icon, label, value, detail, tone = 'default' }: { icon: React.ReactNode; label: string; value: string; detail?: string; tone?: 'default' | 'good' | 'warn' }) {
  return (
    <div className={`runtime-metric runtime-metric--${tone}`}>
      <span className="runtime-metric__icon" aria-hidden>{icon}</span>
      <div className="min-w-0">
        <div className="runtime-metric__label">{label}</div>
        <div className="runtime-metric__value tabular">{value}</div>
        {detail && <div className="runtime-metric__detail">{detail}</div>}
      </div>
    </div>
  );
}

export function RuntimeTelemetry({ onActiveRequestsChange }: { onActiveRequestsChange?: (count: number) => void }) {
  const { t } = useI18n();
  const [data, setData] = useState<RuntimeTelemetrySnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [paused, setPaused] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshMs, setRefreshMs] = useState<number>(REFRESH_OPTIONS[0]);
  const [visible, setVisible] = useState(() => document.visibilityState !== 'hidden');
  const mounted = useRef(false);
  const inFlight = useRef(false);
  const activeRequestsCallback = useRef(onActiveRequestsChange);

  useEffect(() => { activeRequestsCallback.current = onActiveRequestsChange; }, [onActiveRequestsChange]);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  const load = useCallback(async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    setRefreshing(true);
    try {
      const next = await getTelemetry();
      if (mounted.current) {
        setData(next);
        activeRequestsCallback.current?.(next.active_requests);
        setError(null);
      }
    } catch (cause) {
      if (mounted.current) setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      inFlight.current = false;
      if (mounted.current) setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;
    const scheduleNext = () => {
      if (!cancelled && !paused && visible) timer = window.setTimeout(() => void refresh(), refreshMs);
    };
    const refresh = async () => {
      await load();
      scheduleNext();
    };
    const onVisibility = () => setVisible(document.visibilityState !== 'hidden');
    document.addEventListener('visibilitychange', onVisibility);
    if (!paused && visible) void refresh();
    return () => {
      cancelled = true;
      document.removeEventListener('visibilitychange', onVisibility);
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [load, paused, refreshMs, visible]);

  const chart = useMemo(() => {
    const samples = data ? buildTrafficWindow(data.samples, Date.now()) : [];
    const ingress = samples.map((sample) => sample.ingress_bps);
    const egress = samples.map((sample) => sample.egress_bps);
    const max = Math.max(1, ...ingress, ...egress);
    return { ingress, egress, max };
  }, [data]);

  const success = data ? successRate(data) : null;
  const daemonMemory = data?.daemon.memory_bytes ?? null;
  const shellMemory = data?.shell.memory_bytes ?? null;
  const width = 900;
  const height = 170;
  const chartEnd = Date.now();
  const chartStart = chartEnd - TRAFFIC_WINDOW_MS;

  return (
    <section className="runtime-telemetry" aria-labelledby="runtime-telemetry-title">
      <div className="runtime-telemetry__header">
        <div className="flex min-w-0 items-center gap-3">
          <span className="runtime-telemetry__mark" aria-hidden><Activity size={17} /></span>
          <div className="min-w-0">
            <h2 id="runtime-telemetry-title" className="runtime-telemetry__title">{t('telemetry.title')}</h2>
            <p className="runtime-telemetry__subtitle">{t('telemetry.session')}</p>
          </div>
        </div>
        <div className="runtime-telemetry__controls">
          <label className="runtime-telemetry__rate">
            <span>{t('telemetry.refreshRate')}</span>
            <select value={refreshMs} onChange={(event) => setRefreshMs(Number(event.target.value))} aria-label={t('telemetry.refreshRate')}>
              <option value={1000}>{t('telemetry.oneSecond')}</option>
              <option value={5000}>{t('telemetry.fiveSeconds')}</option>
            </select>
          </label>
          <button type="button" className="runtime-telemetry__icon-button" onClick={() => setPaused((value) => !value)} title={paused ? t('telemetry.resume') : t('telemetry.pause')} aria-label={paused ? t('telemetry.resume') : t('telemetry.pause')}>
            {paused ? <Play size={15} /> : <Pause size={15} />}
          </button>
          <button type="button" className="runtime-telemetry__icon-button" onClick={() => void load()} disabled={refreshing} title={t('common.refresh')} aria-label={t('common.refresh')}>
            <RefreshCw size={15} className={refreshing ? 'animate-spin' : undefined} />
          </button>
        </div>
      </div>

      {error && <p role="alert" className="runtime-telemetry__error">{error}</p>}

      <div className="runtime-telemetry__chart-panel">
        <div className="runtime-telemetry__chart-heading">
          <span>{t('telemetry.trafficTitle')}</span>
          <span className="runtime-telemetry__legend"><i className="runtime-legend runtime-legend--in" />{t('telemetry.ingress')} <i className="runtime-legend runtime-legend--out" />{t('telemetry.egress')}</span>
        </div>
        <div className="runtime-telemetry__chart-wrap">
          {chart.ingress.length < 2 ? (
            <div className="runtime-telemetry__empty">{t('telemetry.noData')}</div>
          ) : (
            <svg viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none" role="img" aria-label={t('telemetry.trafficTitle')}>
              {[0.25, 0.5, 0.75].map((fraction) => <line key={fraction} x1="0" x2={width} y1={height * fraction} y2={height * fraction} className="runtime-chart__grid" />)}
              <polyline points={points(chart.ingress, width, height, chart.max)} className="runtime-chart__line runtime-chart__line--in" />
              <polyline points={points(chart.egress, width, height, chart.max)} className="runtime-chart__line runtime-chart__line--out" />
            </svg>
          )}
        </div>
        <div className="runtime-telemetry__chart-footer">
          <span>{data ? new Date(chartStart).toLocaleTimeString() : '—'}</span>
          <span>{data ? t('telemetry.live', { ms: refreshMs }) : '—'}</span>
          <span>{data ? new Date(chartEnd).toLocaleTimeString() : '—'}</span>
        </div>
      </div>

      <div className="runtime-telemetry__metrics">
        <Metric icon={<ArrowUpFromLine size={16} />} label={t('telemetry.ingressRate')} value={data ? formatRate(data.ingress_bps) : '—'} tone="good" />
        <Metric icon={<ArrowDownToLine size={16} />} label={t('telemetry.egressRate')} value={data ? formatRate(data.egress_bps) : '—'} tone="default" />
        <Metric icon={<Cpu size={16} />} label={t('telemetry.daemon')} value={data ? formatBytes(daemonMemory) : '—'} detail={data?.daemon.cpu_percent == null ? t('telemetry.unavailable') : `${formatPercent(data.daemon.cpu_percent)} CPU`} />
        <Metric icon={<HardDrive size={16} />} label={t('telemetry.totalIn')} value={data ? formatBytes(data.ingress_bytes_total) : '—'} />
        <Metric icon={<HardDrive size={16} />} label={t('telemetry.totalOut')} value={data ? formatBytes(data.egress_bytes_total) : '—'} />
      </div>

      <div className="runtime-telemetry__summary">
        <span><strong>{success == null ? '—' : t('telemetry.successRate', { rate: success.toFixed(1) })}</strong></span>
        <span>{t('telemetry.failover')}: <strong>{data?.failover_requests ?? '—'}</strong></span>
        <span>{t('telemetry.gatewayLatency')}: <strong>{formatLatency(data?.avg_gateway_latency_ms)}</strong></span>
        <span>{t('telemetry.upstreamLatency')}: <strong>{formatLatency(data?.avg_upstream_latency_ms)}</strong></span>
      </div>

      <details className="runtime-telemetry__details">
        <summary>{t('telemetry.diagnostics')}</summary>
        <div className="runtime-telemetry__detail-grid">
          <span>{t('telemetry.schema')}: {data?.schema_version ?? '—'}</span>
          <span>{t('telemetry.sampleSource', { source: data?.daemon.source ?? '—', precision: data?.daemon.precision ?? '—' })}</span>
          <span>{t('telemetry.shell')}: {shellMemory == null ? t('telemetry.unavailable') : formatBytes(shellMemory)}</span>
          <span>{t('telemetry.notIncluded')}</span>
        </div>
      </details>
    </section>
  );
}
