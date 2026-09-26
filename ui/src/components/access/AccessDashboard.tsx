import { useCallback, useEffect, useRef, useState } from 'react';
import {
  createCallerToken,
  getAdminActivity,
  getAdminStats,
  getCallerStats,
  getMyActivity,
  getMyCallerStats,
  listCallerTokens,
  revokeCallerToken,
  streamActivity,
  updateCallerToken,
  type ActivityEvent,
  type CallerRole,
  type CallerStats,
  type OwnStats,
  type CallerTokenSummary,
} from '../../api/access';
import { useI18n, type MessageKey } from '../../i18n';
import { useToast } from '../../app/feedback';
import { useAuth } from '../../auth/AuthContext';
import { getCallerToken } from '../../auth/callerSession';
import { parseRequestActivityDetail } from './activityDetail';

type Pane = 'activity' | 'tokens' | 'mine';
type Copy = (key: string, vars?: Record<string, string | number>) => string;

function last30Days() {
  const to = Date.now();
  const from = to - 30 * 24 * 60 * 60 * 1000;
  return { from, to };
}

function formatDate(value: number | null | undefined, never: string): string {
  return value == null ? never : new Date(value).toLocaleString();
}

function formatRate(value: number | null): string {
  return value == null || !Number.isFinite(value) ? '—' : `${(value * 100).toFixed(1)}%`;
}

function eventKindLabel(kind: string, copy: Copy): string {
  switch (kind) {
    case 'request': return copy('access.eventRequest');
    case 'probe': return copy('access.eventProbe');
    case 'config_change': return copy('access.eventConfigChange');
    case 'error': return copy('access.eventError');
    default: return kind;
  }
}

function EventDetails({ event, copy }: { event: ActivityEvent; copy: Copy }) {
  const detail = parseRequestActivityDetail(event.kind, event.detail);
  let rawDetail = event.detail;
  try {
    rawDetail = JSON.stringify(JSON.parse(event.detail), null, 2) ?? event.detail;
  } catch {
    // Keep malformed legacy text available in the disclosure.
  }

  const rawDisclosure = (
    <details className="mt-2 text-xs">
      <summary className="w-fit cursor-pointer select-none" style={{ color: 'var(--text-muted)' }}>
        {copy(event.kind === 'request' ? 'access.rawJson' : 'access.rawDetails')}
      </summary>
      <pre className="mt-2 max-h-64 overflow-auto whitespace-pre-wrap break-words rounded-md border p-2" style={{ borderColor: 'var(--border)', background: 'var(--surface-hover)', color: 'var(--text)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)' }}>
        {rawDetail}
      </pre>
    </details>
  );

  if (!detail) {
    return (
      <div className="min-w-48">
        <span style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>{copy('access.summaryUnavailable')}</span>
        {rawDisclosure}
      </div>
    );
  }

  const statusLabel = detail.success === null || detail.status === null
    ? null
    : copy(detail.success ? 'access.requestSucceeded' : 'access.requestFailed', { status: detail.status });
  const statusStyle: React.CSSProperties = {
    color: detail.success ? 'var(--success)' : 'var(--danger)',
    background: detail.success ? 'var(--success-bg)' : 'var(--danger-bg)',
  };
  const route = detail.provider || detail.upstreamModel
    ? `${detail.provider ?? '—'} → ${detail.upstreamModel ?? '—'}`
    : copy('access.routeUnknown');
  const cost = detail.costUsd === null ? copy('access.noCost') : `${detail.costUsd.toFixed(6)} USD`;
  const callsKey = detail.upstreamCalls === 1 ? 'access.upstreamCall' : 'access.upstreamCalls';

  return (
    <div className="min-w-48 space-y-1.5">
      <div className="flex flex-wrap items-center gap-2">
        <span className="font-semibold" style={{ color: 'var(--text)' }}>{detail.endpointId ?? copy('access.unknownEndpoint')}</span>
        {statusLabel && <span className="rounded-full px-2 py-0.5 text-xs font-medium" style={statusStyle}>{statusLabel}</span>}
        <span className="tabular" style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>
          {detail.latencyMs === null ? '—' : `${detail.latencyMs} ms`}
        </span>
      </div>
      {detail.requestId && <div className="text-xs" style={{ color: 'var(--text-muted)' }}>{copy('access.requestId')}: <code className="break-all">{detail.requestId}</code></div>}
      <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs">
        <span><span style={{ color: 'var(--text-muted)' }}>{copy('access.route')}: </span>{route}</span>
        <span style={{ color: 'var(--text-muted)' }}>
          {copy('access.tokenUsage', { input: detail.inputTokens ?? '—', output: detail.outputTokens ?? '—' })}
        </span>
        <span style={{ color: 'var(--text-muted)' }}>
          {copy(callsKey, { count: detail.upstreamCalls ?? '—' })}
        </span>
        <span style={{ color: 'var(--text-muted)' }}>{copy('access.costValue', { value: cost })}</span>
      </div>
      {rawDisclosure}
    </div>
  );
}

function StatGrid({ stats, copy }: { stats: CallerStats | OwnStats | null; copy: Copy }) {
  const item = (label: string, value: string) => (
    <div className="rounded-md p-3" style={{ background: 'var(--surface-hover)' }} key={label}>
      <div style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>{label}</div>
      <div className="mt-1 font-semibold tabular" style={{ fontSize: 'var(--text-base)' }}>{value}</div>
    </div>
  );
  return (
    <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
      {item(copy('access.requests'), stats ? String(stats.total_requests) : '—')}
      {item(copy('access.cost'), stats?.total_cost == null ? copy('access.noCost') : stats.total_cost.toFixed(6))}
      {item(copy('access.avgLatency'), stats?.avg_latency_ms == null ? '—' : `${stats.avg_latency_ms.toFixed(0)} ms`)}
      {item(copy('access.errorRate'), formatRate(stats?.error_rate ?? null))}
    </div>
  );
}

function TokenManager({ copy }: { copy: Copy }) {
  const { toast } = useToast();
  const [tokens, setTokens] = useState<CallerTokenSummary[]>([]);
  const [selectedId, setSelectedId] = useState('');
  const [stats, setStats] = useState<OwnStats | null>(null);
  const [statsError, setStatsError] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [hasLoaded, setHasLoaded] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [statsRefresh, setStatsRefresh] = useState(0);
  const hasLoadedRef = useRef(false);
  const statsOwnerRef = useRef('');
  const [busyId, setBusyId] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [role, setRole] = useState<CallerRole>('readonly');
  const [creating, setCreating] = useState(false);
  const [newSecret, setNewSecret] = useState<string | null>(null);
  const [revokeId, setRevokeId] = useState<string | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  const refresh = useCallback(async () => {
    setLoadError(null);
    setStatsRefresh((value) => value + 1);
    if (hasLoadedRef.current) setRefreshing(true);
    try {
      const result = await listCallerTokens();
      setTokens(result.tokens);
      setSelectedId((current) => current && result.tokens.some((token) => token.id === current)
        ? current
        : result.tokens[0]?.id ?? '');
    } catch (error) {
      setLoadError((error as Error).message);
    } finally {
      hasLoadedRef.current = true;
      setHasLoaded(true);
      setRefreshing(false);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  useEffect(() => {
    if (!selectedId) {
      setStats(null);
      setStatsError(null);
      statsOwnerRef.current = '';
      return;
    }
    let live = true;
    if (statsOwnerRef.current !== selectedId) {
      statsOwnerRef.current = selectedId;
      setStats(null);
    }
    const { from, to } = last30Days();
    setStatsError(null);
    void getCallerStats(selectedId, from, to).then((result) => {
      if (live) setStats(result);
    }).catch((error) => {
      if (live) setStatsError((error as Error).message);
    });
    return () => { live = false; };
  }, [selectedId, statsRefresh]);

  const create = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!name.trim() || creating) return;
    setCreating(true);
    try {
      const result = await createCallerToken({ name: name.trim(), role });
      setTokens((current) => [...current.filter((item) => item.id !== result.token.id), result.token]);
      setSelectedId(result.token.id);
      setName('');
      // In-memory only, until the user closes the one-time display.
      setNewSecret(result.secret);
      panelRef.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      toast('ok', copy('access.secretTitle'));
    } catch (error) {
      toast('danger', copy('access.createFailed', { reason: (error as Error).message }));
    } finally {
      setCreating(false);
    }
  };

  const update = async (token: CallerTokenSummary, patch: { role?: CallerRole; enabled?: boolean }) => {
    setBusyId(token.id);
    try {
      const result = await updateCallerToken(token.id, patch);
      setTokens((current) => current.map((item) => item.id === token.id ? result.token : item));
    } catch (error) {
      toast('danger', copy('access.updateFailed', { reason: (error as Error).message }));
      void refresh();
    } finally {
      setBusyId(null);
    }
  };

  const revoke = async (id: string) => {
    setBusyId(id);
    try {
      await revokeCallerToken(id);
      setTokens((current) => current.filter((item) => item.id !== id));
      setRevokeId(null);
      if (selectedId === id) setSelectedId('');
    } catch (error) {
      toast('danger', copy('access.revokeFailed', { reason: (error as Error).message }));
    } finally {
      setBusyId(null);
    }
  };

  const card: React.CSSProperties = {
    background: 'var(--surface)',
    border: '1px solid var(--border)',
    borderRadius: 'var(--radius)',
  };
  const field: React.CSSProperties = {
    border: '1px solid var(--border)', borderRadius: 'var(--radius)', background: 'var(--surface)',
    color: 'var(--text)', padding: '0.5rem 0.65rem', fontSize: 'var(--text-sm)',
  };
  const button: React.CSSProperties = {
    border: '1px solid var(--border)', borderRadius: 'var(--radius)', background: 'var(--surface-hover)',
    color: 'var(--text)', padding: '0.4rem 0.65rem', fontSize: 'var(--text-sm)',
  };

  return (
    <div ref={panelRef} className="space-y-5">
      <div>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h2 className="font-semibold" style={{ fontSize: 'var(--text-lg)' }}>{copy('access.tokenTitle')}</h2>
          <button type="button" disabled={!hasLoaded || refreshing} onClick={() => void refresh()} className="rounded-md px-3 py-1.5 text-sm" style={{ background: 'var(--surface-hover)' }}>
            {refreshing ? copy('access.refreshing') : copy('access.refresh')}
          </button>
        </div>
        <p className="mt-1" style={{ color: 'var(--text-muted)', fontSize: 'var(--text-sm)' }}>{copy('access.tokenHint')}</p>
      </div>

      <form onSubmit={(event) => void create(event)} className="flex flex-wrap items-end gap-3 rounded-md p-4" style={card}>
        <label className="flex min-w-48 flex-1 flex-col gap-1" style={{ fontSize: 'var(--text-sm)' }}>
          {copy('access.tokenName')}
          <input value={name} onChange={(event) => setName(event.target.value)} maxLength={80} required style={field} />
        </label>
        <label className="flex flex-col gap-1" style={{ fontSize: 'var(--text-sm)' }}>
          {copy('access.role')}
          <select value={role} onChange={(event) => setRole(event.target.value as CallerRole)} style={field}>
            <option value="readonly">{copy('access.roleReadonly')}</option>
            <option value="admin">{copy('access.roleAdmin')}</option>
          </select>
        </label>
        <button type="submit" disabled={creating || !name.trim()} style={{ ...button, background: 'var(--accent)', borderColor: 'var(--accent)', color: '#fff' }}>
          {creating ? copy('instance.saving') : copy('access.newToken')}
        </button>
      </form>

      {loadError && <p role="alert" style={{ color: 'var(--danger)', fontSize: 'var(--text-sm)' }}>{copy('access.loadFailed', { reason: loadError })}</p>}
      {!hasLoaded && <p role="status" style={{ color: 'var(--text-muted)', fontSize: 'var(--text-sm)' }}>{copy('access.loadingTokens')}</p>}
      <div className="space-y-3">
        {tokens.map((token) => (
          <article key={token.id} className="rounded-md p-4" style={{ ...card, opacity: token.enabled ? 1 : 0.72 }}>
            <div className="flex flex-wrap items-start justify-between gap-4">
              <div className="min-w-48 flex-1">
                <button type="button" onClick={() => setSelectedId(token.id)} className="font-mono font-semibold" style={{ color: 'var(--accent)', textAlign: 'left' }}>
                  {token.name}
                </button>
                <div className="mt-1 font-mono text-xs" style={{ color: 'var(--text-muted)' }}>{token.id}</div>
                <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1" style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>
                  <span>{copy('access.created')}: {formatDate(token.created_at, '—')}</span>
                  <span>{copy('access.lastUsed')}: {formatDate(token.last_used_at, copy('access.neverUsed'))}</span>
                </div>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <label className="flex items-center gap-2" style={{ fontSize: 'var(--text-sm)' }}>
                  <span>{copy('access.role')}</span>
                  <select
                    aria-label={`${copy('access.role')} ${token.name}`}
                    value={token.role}
                    disabled={busyId === token.id}
                    onChange={(event) => void update(token, { role: event.target.value as CallerRole })}
                    style={field}
                  >
                    <option value="readonly">{copy('access.roleReadonly')}</option>
                    <option value="admin">{copy('access.roleAdmin')}</option>
                  </select>
                </label>
                <label className="flex items-center gap-2" style={{ fontSize: 'var(--text-sm)' }}>
                  <input
                    type="checkbox"
                    checked={token.enabled}
                    disabled={busyId === token.id}
                    onChange={(event) => void update(token, { enabled: event.target.checked })}
                  />
                  {copy(token.enabled ? 'access.enabled' : 'access.disabled')}
                </label>
                {revokeId === token.id ? (
                  <>
                    <span role="status" style={{ color: 'var(--warning)', fontSize: 'var(--text-xs)' }}>{copy('access.confirmRevoke')}</span>
                    <button type="button" disabled={busyId === token.id} onClick={() => void revoke(token.id)} style={{ ...button, color: 'var(--danger)' }}>{copy('access.revoke')}</button>
                    <button type="button" disabled={busyId === token.id} onClick={() => setRevokeId(null)} style={button}>{copy('access.keep')}</button>
                  </>
                ) : (
                  <button type="button" disabled={busyId === token.id} onClick={() => setRevokeId(token.id)} style={{ ...button, color: 'var(--danger)' }}>{copy('access.revoke')}</button>
                )}
              </div>
            </div>
          </article>
        ))}
        {hasLoaded && !loadError && tokens.length === 0 && <p style={{ color: 'var(--text-muted)', fontSize: 'var(--text-sm)' }}>{copy('access.noTokens')}</p>}
      </div>

      {selectedId && (
        <section className="space-y-3 rounded-md p-4" style={card}>
          <div className="flex flex-wrap items-baseline justify-between gap-2">
            <h3 className="font-semibold" style={{ fontSize: 'var(--text-base)' }}>{copy('access.statsTitle')}</h3>
            <span style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>{copy('access.last30Days')}</span>
          </div>
          {statsError && <p role="alert" style={{ color: 'var(--danger)', fontSize: 'var(--text-sm)' }}>{copy('access.eventsFailed', { reason: statsError })}</p>}
          <StatGrid stats={stats} copy={copy} />
        </section>
      )}

      {newSecret && <OneTimeSecret secret={newSecret} copy={copy} onClose={() => setNewSecret(null)} />}
    </div>
  );
}

function OneTimeSecret({ secret, copy, onClose }: { secret: string; copy: Copy; onClose: () => void }) {
  const { toast } = useToast();
  const [copied, setCopied] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    if (dialog.current && !dialog.current.open) dialog.current.showModal();
  }, []);
  const close = () => {
    if (dialog.current?.open) dialog.current.close();
    onClose();
  };
  const style: React.CSSProperties = {
    background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 'var(--radius)',
    color: 'var(--text)', width: 'min(38rem, calc(100vw - 2rem))', padding: '1.25rem',
  };
  return (
    <dialog ref={dialog} onCancel={(event) => { event.preventDefault(); close(); }} className="responsive-dialog" style={style}>
      <h2 className="font-semibold" style={{ fontSize: 'var(--text-lg)' }}>{copy('access.secretTitle')}</h2>
      <p className="mt-2" style={{ color: 'var(--text-muted)', fontSize: 'var(--text-sm)' }}>{copy('access.secretHint')}</p>
      <textarea readOnly value={secret} rows={3} className="mt-4 w-full resize-y rounded-md p-3 font-mono text-sm" onFocus={(event) => event.currentTarget.select()} />
      <div className="mt-4 flex flex-wrap justify-end gap-2">
        <button type="button" onClick={async () => {
          try {
            await navigator.clipboard.writeText(secret);
            setCopied(true);
            toast('ok', copy('access.copied'));
          } catch {
            toast('warn', copy('access.copyFailed'));
          }
        }} className="rounded-md px-3 py-2 text-sm" style={{ background: 'var(--surface-hover)' }}>{copied ? copy('access.copied') : copy('access.copySecret')}</button>
        <button type="button" onClick={close} className="rounded-md px-3 py-2 text-sm" style={{ background: 'var(--accent)', color: '#fff' }}>{copy('access.close')}</button>
      </div>
    </dialog>
  );
}

function ActivityFeed({ callerToken, copy }: { callerToken?: string; copy: Copy }) {
  const [events, setEvents] = useState<ActivityEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [streaming, setStreaming] = useState(false);
  const [initialLoading, setInitialLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const cursorRef = useRef(0);
  const loadedRef = useRef(false);
  const mergeEvents = useCallback((incoming: ActivityEvent[]) => {
    if (!incoming.length) return;
    cursorRef.current = Math.max(cursorRef.current, ...incoming.map((event) => event.id));
    setEvents((current) => {
      const merged = new Map(current.map((event) => [event.id, event]));
      for (const event of incoming) merged.set(event.id, event);
      return [...merged.values()].sort((a, b) => b.id - a.id).slice(0, 50);
    });
  }, []);

  useEffect(() => {
    let active = true;
    cursorRef.current = 0;
    loadedRef.current = false;
    setEvents([]);
    setError(null);
    setStreaming(false);
    setInitialLoading(true);
    setRefreshing(false);
    const refresh = async () => {
      if (loadedRef.current) setRefreshing(true);
      try {
        const result = callerToken
          ? await getMyActivity(callerToken, cursorRef.current)
          : await getAdminActivity(cursorRef.current);
        if (!active) return;
        mergeEvents(result.events);
        cursorRef.current = Math.max(cursorRef.current, result.next_since);
        setError(null);
      } catch (cause) {
        if (active) setError((cause as Error).message);
      } finally {
        if (active) {
          loadedRef.current = true;
          setInitialLoading(false);
          setRefreshing(false);
        }
      }
    };
    void refresh();
    const timer = window.setInterval(() => void refresh(), 15000);
    const controller = new AbortController();
    void streamActivity(callerToken ? { type: 'caller', token: callerToken } : { type: 'admin' }, cursorRef.current, controller.signal, (event) => {
      if (active) mergeEvents([event]);
    }).then(() => {
      if (active && !controller.signal.aborted) setStreaming(false);
    }).catch((cause) => {
      if (active && !controller.signal.aborted) setStreaming(false);
      if (active && !controller.signal.aborted && !(cause instanceof DOMException && cause.name === 'AbortError')) {
        // The polling endpoint remains the fallback if streaming is unavailable.
      }
    });
    setStreaming(true);
    return () => {
      active = false;
      window.clearInterval(timer);
      controller.abort();
    };
  }, [callerToken, mergeEvents]);

  const refreshNow = async () => {
    setRefreshing(true);
    try {
      const result = callerToken
        ? await getMyActivity(callerToken, cursorRef.current)
        : await getAdminActivity(cursorRef.current);
      mergeEvents(result.events);
      cursorRef.current = Math.max(cursorRef.current, result.next_since);
      setError(null);
    } catch (cause) {
      setError((cause as Error).message);
    } finally {
      setRefreshing(false);
    }
  };

  const card: React.CSSProperties = {
    background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 'var(--radius)',
  };
  return (
    <section className="space-y-3 rounded-md p-4" style={card}>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h3 className="font-semibold" style={{ fontSize: 'var(--text-base)' }}>{copy('access.eventsTitle')}</h3>
        <div className="flex items-center gap-3">
          <span style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>
            {streaming ? copy('access.live') : copy('access.polling')}
          </span>
          <button type="button" disabled={refreshing} onClick={() => void refreshNow()} className="rounded-md px-3 py-1.5 text-sm" style={{ background: 'var(--surface-hover)' }}>{refreshing ? copy('access.refreshing') : copy('access.refresh')}</button>
        </div>
      </div>
      {error && <p role="alert" style={{ color: 'var(--danger)', fontSize: 'var(--text-sm)' }}>{copy(callerToken ? 'access.myLoadFailed' : 'access.eventsFailed', { reason: error })}</p>}
      {!streaming && <p style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>{copy('access.streamPaused')}</p>}
      <div className="overflow-x-auto">
        <table className="w-full border-collapse text-left text-sm">
          <thead style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>
            <tr className="border-b" style={{ borderColor: 'var(--border)' }}>
              <th className="px-2 py-2 text-xs font-medium sm:text-sm">{copy('access.time')}</th>
              <th className="hidden px-2 py-2 font-medium sm:table-cell">{copy('access.kind')}</th>
              {!callerToken && <th className="hidden px-2 py-2 font-medium lg:table-cell">{copy('access.callToken')}</th>}
              <th className="px-2 py-2 font-medium">{copy('access.details')}</th>
            </tr>
          </thead>
          <tbody>
            {events.map((event) => (
              <tr key={event.id} className="border-b align-top" style={{ borderColor: 'var(--border)' }}>
                <td className="whitespace-nowrap px-2 py-2 text-xs tabular sm:text-sm" style={{ color: 'var(--text-muted)' }}>{formatDate(event.timestamp, '—')}</td>
                <td className="hidden whitespace-nowrap px-2 py-2 sm:table-cell">{eventKindLabel(event.kind, copy)}</td>
                {!callerToken && <td className="hidden px-2 py-2 font-mono text-xs lg:table-cell">{event.token_id ?? '—'}</td>}
                <td className="max-w-xl px-2 py-2">
                  <div className="mb-1 flex flex-wrap items-center gap-x-2 text-xs sm:hidden" style={{ color: 'var(--text-muted)' }}>
                    <span>{eventKindLabel(event.kind, copy)}</span>
                    {!callerToken && <span className="max-w-full break-all font-mono">{event.token_id ?? '—'}</span>}
                  </div>
                  <EventDetails event={event} copy={copy} />
                </td>
              </tr>
            ))}
            {initialLoading && events.length === 0 && <tr><td colSpan={callerToken ? 3 : 4} className="px-2 py-6 text-center" style={{ color: 'var(--text-muted)' }}>{copy('access.loadingEvents')}</td></tr>}
            {!initialLoading && events.length === 0 && !error && <tr><td colSpan={callerToken ? 3 : 4} className="px-2 py-6 text-center" style={{ color: 'var(--text-muted)' }}>{copy('access.noEvents')}</td></tr>}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function MyUsage({ copy }: { copy: Copy }) {
  const auth = useAuth();
  const [input, setInput] = useState('');
  const [activeToken, setActiveToken] = useState('');
  const [stats, setStats] = useState<OwnStats | null>(null);
  const [allStats, setAllStats] = useState<CallerStats[]>([]);
  const [callerRole, setCallerRole] = useState<CallerRole | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const hydratedTokenRef = useRef<string | null>(null);
  const field: React.CSSProperties = { border: '1px solid var(--border)', borderRadius: 'var(--radius)', background: 'var(--surface)', color: 'var(--text)', padding: '0.5rem 0.65rem', fontSize: 'var(--text-sm)' };
  const loadForIdentity = async (token: string, role: CallerRole) => {
    setLoading(true);
    setError(null);
    const { from, to } = last30Days();
    try {
      if (role === 'admin') {
        const result = await getAdminStats(from, to, token);
        setAllStats(result.tokens);
        setStats(null);
      } else {
        const result = await getMyCallerStats(token, from, to);
        setStats(result);
        setAllStats([]);
      }
      setCallerRole(role);
      setActiveToken(token);
      hydratedTokenRef.current = token;
    } catch (cause) {
      setStats(null);
      setAllStats([]);
      setActiveToken('');
      setError((cause as Error).message);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    const token = getCallerToken();
    if (auth.identity?.kind === 'caller' && token && hydratedTokenRef.current !== token) {
      void loadForIdentity(token, auth.identity.role);
    }
    // Hydrate the in-memory caller identity when this internal view is first opened.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [auth.identity?.kind, auth.identity?.kind === 'caller' ? auth.identity.role : null]);

  const load = async (event: React.FormEvent) => {
    event.preventDefault();
    const token = input.trim();
    if (!token || loading) {
      if (!token) setError(copy('access.tokenRequired'));
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const role = await auth.loginCaller(token);
      await loadForIdentity(token, role);
    } catch (cause) {
      setStats(null);
      setAllStats([]);
      setActiveToken('');
      setError((cause as Error).message);
    }
    setLoading(false);
  };
  const clear = () => {
    setInput('');
    setActiveToken('');
    hydratedTokenRef.current = null;
    setStats(null);
    setAllStats([]);
    setCallerRole(null);
    setError(null);
    if (auth.identity?.kind === 'caller') auth.logoutCaller();
  };
  return (
    <div className="space-y-4">
      <section className="rounded-md p-4" style={{ background: 'var(--surface)', border: '1px solid var(--border)' }}>
        <h2 className="font-semibold" style={{ fontSize: 'var(--text-lg)' }}>{copy('access.viewOwn')}</h2>
        <p className="mt-1" style={{ color: 'var(--text-muted)', fontSize: 'var(--text-sm)' }}>{copy('access.myDataHint')}</p>
        {auth.identity?.kind !== 'caller' && (
          <form onSubmit={(event) => void load(event)} className="mt-4 flex flex-wrap gap-2">
            <label className="sr-only" htmlFor="my-caller-token">{copy('access.callToken')}</label>
            <input id="my-caller-token" type="password" autoComplete="off" spellCheck={false} value={input} onChange={(event) => setInput(event.target.value)} placeholder={copy('access.callToken')} style={{ ...field, minWidth: '16rem', flex: '1 1 20rem', fontFamily: 'var(--font-mono)' }} />
            <button type="submit" disabled={loading || !input.trim()} className="rounded-md px-3 py-2 text-sm" style={{ background: 'var(--accent)', color: '#fff' }}>{loading ? copy('instance.saving') : copy('access.loadMine')}</button>
          </form>
        )}
        {auth.identity?.kind === 'caller' && (
          <div className="mt-3 flex flex-wrap items-center gap-3" style={{ fontSize: 'var(--text-sm)' }}>
            <span className="font-mono" style={{ color: 'var(--text-muted)' }}>{auth.identity.id} · {copy(auth.identity.role === 'admin' ? 'access.roleAdmin' : 'access.roleReadonly')}</span>
            <button type="button" onClick={clear} className="rounded-md px-3 py-2 text-sm" style={{ background: 'var(--surface-hover)' }}>{copy('access.clearToken')}</button>
          </div>
        )}
        {auth.identity?.kind === 'caller' && (
          <button type="button" disabled={loading} onClick={() => {
            const token = getCallerToken();
            if (token && auth.identity?.kind === 'caller') void loadForIdentity(token, auth.identity.role);
          }} className="mt-3 rounded-md px-3 py-2 text-sm" style={{ background: 'var(--surface-hover)' }}>
            {copy('access.refresh')}
          </button>
        )}
        {error && <p role="alert" className="mt-3" style={{ color: 'var(--danger)', fontSize: 'var(--text-sm)' }}>{copy('access.myLoadFailed', { reason: error })}</p>}
      </section>
      {activeToken && (
        <>
          {callerRole === 'admin' ? (
            <section className="space-y-3 rounded-md p-4" style={{ background: 'var(--surface)', border: '1px solid var(--border)' }}>
              <div className="flex flex-wrap items-baseline justify-between gap-2">
                <h3 className="font-semibold" style={{ fontSize: 'var(--text-base)' }}>{copy('access.topTokens')}</h3>
                <span style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>{copy('access.last30Days')}</span>
              </div>
              {allStats.length === 0 ? copy('access.noEvents') : (
                <div className="overflow-x-auto">
                  <table className="w-full border-collapse text-left text-sm">
                    <thead style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}><tr><th className="px-2 py-2">{copy('access.callToken')}</th><th className="px-2 py-2">{copy('access.requests')}</th><th className="px-2 py-2">{copy('access.cost')}</th><th className="px-2 py-2">{copy('access.avgLatency')}</th><th className="px-2 py-2">{copy('access.errorRate')}</th></tr></thead>
                    <tbody>{[...allStats].sort((a, b) => b.total_requests - a.total_requests).slice(0, 5).map((row) => <tr key={row.token_id} className="border-t" style={{ borderColor: 'var(--border)' }}><td className="px-2 py-2 font-mono">{row.name ?? row.token_id ?? '—'}</td><td className="px-2 py-2 tabular">{row.total_requests}</td><td className="px-2 py-2 tabular">{row.total_cost == null ? copy('access.noCost') : row.total_cost.toFixed(6)}</td><td className="px-2 py-2 tabular">{row.avg_latency_ms == null ? '—' : `${row.avg_latency_ms.toFixed(0)} ms`}</td><td className="px-2 py-2 tabular">{formatRate(row.error_rate)}</td></tr>)}</tbody>
                  </table>
                </div>
              )}
            </section>
          ) : (
            <section className="space-y-3 rounded-md p-4" style={{ background: 'var(--surface)', border: '1px solid var(--border)' }}>
              <div className="flex flex-wrap items-baseline justify-between gap-2">
                <h3 className="font-semibold" style={{ fontSize: 'var(--text-base)' }}>{copy('access.myStats')}</h3>
                <span style={{ color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>{copy('access.last30Days')}</span>
              </div>
              <StatGrid stats={stats} copy={copy} />
            </section>
          )}
          <ActivityFeed key={activeToken} callerToken={callerRole === 'readonly' ? activeToken : undefined} copy={copy} />
        </>
      )}
    </div>
  );
}

export function AccessDashboard() {
  const { t } = useI18n();
  const auth = useAuth();
  const identityKey = auth.identity
    ? `${auth.identity.kind}:${auth.identity.kind === 'caller' ? auth.identity.id : ''}:${auth.identity.role}`
    : 'anonymous';
  const copy: Copy = (key, vars) => t(key as MessageKey, vars);
  const [pane, setPane] = useState<Pane>(auth.isReadOnly ? 'mine' : 'activity');
  useEffect(() => {
    if (auth.isReadOnly) setPane('mine');
  }, [auth.isReadOnly]);
  const tabStyle = (active: boolean): React.CSSProperties => ({
    border: '1px solid var(--border)', borderRadius: 'var(--radius)', padding: '0.5rem 0.8rem',
    color: active ? '#fff' : 'var(--text)', background: active ? 'var(--accent)' : 'var(--surface)',
    fontSize: 'var(--text-sm)',
  });

  const tabs = [
    ['activity', 'access.activityTab'], ['tokens', 'access.tokensTab'], ['mine', 'access.myTab'],
  ] as const;
  return (
    <section className="fade-in mb-6 space-y-4 rounded-md p-5" style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 'var(--radius)' }}>
      <h2 className="font-semibold" style={{ fontSize: 'var(--text-xl)' }}>{copy('access.title')}</h2>
      <nav className="flex flex-wrap gap-2" aria-label={copy('access.title')}>
        {tabs.filter(([id]) => auth.isReadOnly ? id === 'mine' : true).map(([id, key]) => (
          <button type="button" key={id} aria-pressed={pane === id} onClick={() => setPane(id)} style={tabStyle(pane === id)}>
            {copy(key)}
          </button>
        ))}
      </nav>
      {pane === 'activity' && <ActivityFeed key={identityKey} copy={copy} />}
      {pane === 'tokens' && auth.canManage && <TokenManager key={identityKey} copy={copy} />}
      {pane === 'mine' && <MyUsage key={identityKey} copy={copy} />}
    </section>
  );
}
