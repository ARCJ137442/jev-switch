import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { JevRequest, JevResponse, ModelEntry } from '../../api';
import { SystemOneError, postSystemOne } from '../../api';
import { AdminApiError, type AdminProvider } from '../../api/admin';
import { invokeProviderModel } from '../../api/playground';
import { useI18n, type MessageKey } from '../../i18n';
import { useAuth } from '../../auth/AuthContext';
import { localizePlayground, type PlaygroundCopy } from '../../i18n/playground';
import { QuestionFormEditor } from './QuestionFormEditor';
import { ComparisonQueue } from './comparisonQueue';
import { getSnapshot, type ComparisonSnapshot as Snapshot } from './comparisonSnapshot';
import './comparison.css';

export type ComparisonTarget =
  | { key: string; kind: 'public'; modelId: string; discovered?: boolean }
  | { key: string; kind: 'direct'; providerId: string; providerLabel: string; modelId: string; discovered: boolean };

type Status = 'idle' | 'queued' | 'loading' | 'ok' | 'error' | 'cancelled';
interface Attempt {
  requestId: string;
  attemptId: string;
  target: ComparisonTarget;
  snapshot: Snapshot;
  status: Status;
  response?: JevResponse;
  error?: string;
  durationMs?: number;
  startedAt?: number;
  finishedAt?: number;
}
interface QueueJobPayload { attempt: Attempt }
interface RunResult { response: JevResponse; durationMs: number; finishedAt: number }

interface Props {
  stateJson: string;
  questionsJson: string;
  onStateChange: (value: string) => void;
  onQuestionsChange: (value: string) => void;
  publicModels: ModelEntry[];
  publicModelStatus: 'loading' | 'ready' | 'empty' | 'unauthorized' | 'forbidden' | 'network-error' | 'request-error';
  providers: AdminProvider[];
  providersLoading: boolean;
  providersError: boolean;
  targets: ComparisonTarget[];
  onTargetsChange: (targets: ComparisonTarget[]) => void;
}

const cardStyle: React.CSSProperties = {
  background: 'var(--surface)',
  border: '1px solid var(--border)',
  borderRadius: 'var(--radius)',
};
const codeStyle: React.CSSProperties = {
  border: '1px solid var(--border)',
  borderRadius: 'var(--radius)',
  background: 'var(--surface-hover)',
  color: 'var(--text)',
  fontFamily: 'var(--font-mono)',
  fontSize: 'var(--text-xs)',
};
const smallLabel: React.CSSProperties = { color: 'var(--text-muted)', fontSize: 'var(--text-xs)' };
const buttonStyle: React.CSSProperties = {
  minHeight: 32,
  padding: '0 10px',
  border: '1px solid var(--border)',
  borderRadius: 'var(--radius)',
  color: 'var(--text)',
  background: 'var(--surface)',
  fontSize: 'var(--text-sm)',
};

function uid(): string {
  return typeof crypto !== 'undefined' && 'randomUUID' in crypto
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function isCompactViewport(): boolean {
  return typeof window !== 'undefined' && window.matchMedia('(max-width: 799px), (max-height: 600px)').matches;
}

function objectKeys(source: string): string[] {
  try {
    const value: unknown = JSON.parse(source);
    return value !== null && typeof value === 'object' && !Array.isArray(value) ? Object.keys(value) : [];
  } catch {
    return [];
  }
}

function targetLabel(target: ComparisonTarget, copy: PlaygroundCopy): string {
  return target.kind === 'public'
    ? `${target.modelId}${target.discovered ? '' : ` · ${copy.manual}`}`
    : `${target.providerLabel} · ${target.modelId}${target.discovered ? '' : ` · ${copy.manual}`}`;
}

export function ComparisonPanel({
  stateJson,
  questionsJson,
  onStateChange,
  onQuestionsChange,
  publicModels,
  publicModelStatus,
  providers,
  providersLoading,
  providersError,
  targets,
  onTargetsChange,
}: Props) {
  const { t } = useI18n();
  const auth = useAuth();
  const copy = useMemo(() => localizePlayground((key) => t(key as MessageKey)), [t]);
  const [inputMode, setInputMode] = useState<'form' | 'json'>('form');
  const [compactLayout, setCompactLayout] = useState(isCompactViewport);
  const [inputsOpen, setInputsOpen] = useState(() => !isCompactViewport());
  const [entryMode, setEntryMode] = useState<'public' | 'direct'>('public');
  const [publicToAdd, setPublicToAdd] = useState('');
  const [providerToAdd, setProviderToAdd] = useState('');
  const [upstreamModel, setUpstreamModel] = useState('');
  const [attempts, setAttempts] = useState<Attempt[]>([]);
  const [validationError, setValidationError] = useState<string | null>(null);
  const [formValid, setFormValid] = useState(true);
  const [runningCount, setRunningCount] = useState(0);
  const attemptsRef = useRef<Attempt[]>([]);
  const mountedRef = useRef(true);
  const MAX_CONCURRENT = 3;

  useEffect(() => {
    const viewport = window.matchMedia('(max-width: 799px), (max-height: 600px)');
    const syncCompactLayout = () => {
      setCompactLayout(viewport.matches);
      setInputsOpen(!viewport.matches);
    };
    viewport.addEventListener('change', syncCompactLayout);
    return () => viewport.removeEventListener('change', syncCompactLayout);
  }, []);

  const stateFields = useMemo(() => objectKeys(stateJson), [stateJson]);
  const questionCount = useMemo(() => objectKeys(questionsJson).length, [questionsJson]);

  const updateAttempt = useCallback((attemptId: string, update: (attempt: Attempt) => Attempt) => {
    const index = attemptsRef.current.findIndex((attempt) => attempt.attemptId === attemptId);
    if (index < 0) return;
    const next = attemptsRef.current.slice();
    next[index] = update(next[index]);
    attemptsRef.current = next;
    if (mountedRef.current) {
      setAttempts(next);
      setRunningCount(next.filter((attempt) => attempt.status === 'queued' || attempt.status === 'loading').length);
    }
  }, []);

  const createQueue = useCallback(() => new ComparisonQueue<QueueJobPayload, RunResult>(MAX_CONCURRENT, {
    onStatus: (job, status, value) => {
      updateAttempt(job.attemptId, (current) => {
        if (status === 'loading') return { ...current, status, startedAt: Date.now() };
        if (status === 'queued' || status === 'cancelled') return { ...current, status, finishedAt: status === 'cancelled' ? Date.now() : current.finishedAt };
        if (status === 'ok' && value && !(value instanceof Error)) return { ...current, status, response: value.response, durationMs: value.durationMs, finishedAt: value.finishedAt };
        if (status === 'error') return { ...current, status, error: value instanceof Error ? formatError(value, copy) : String(value ?? '') };
        return current;
      });
    },
  }), [updateAttempt, copy]);
  const queueRef = useRef<ComparisonQueue<QueueJobPayload, RunResult> | null>(null);
  useEffect(() => {
    mountedRef.current = true;
    queueRef.current = createQueue();
    return () => {
      mountedRef.current = false;
      queueRef.current?.dispose();
      queueRef.current = null;
    };
  }, [createQueue]);

  useEffect(() => {
    if (!auth.isReadOnly) return;
    const directKeys = new Set(targets.filter((target) => target.kind === 'direct').map((target) => target.key));
    if (directKeys.size === 0) return;
    for (const attempt of attemptsRef.current) {
      if (attempt.target.kind === 'direct') queueRef.current?.cancel(attempt.target.key, attempt.attemptId);
    }
    const publicTargets = targets.filter((target) => target.kind === 'public');
    const publicAttempts = attemptsRef.current.filter((attempt) => attempt.target.kind === 'public');
    attemptsRef.current = publicAttempts;
    setAttempts(publicAttempts);
    setRunningCount(publicAttempts.filter((attempt) => attempt.status === 'queued' || attempt.status === 'loading').length);
    onTargetsChange(publicTargets);
  }, [auth.isReadOnly, attemptsRef, onTargetsChange, targets]);

  const enqueue = useCallback((targetsToRun: ComparisonTarget[], snapshot: Snapshot, previous?: Attempt[]) => {
    const requestId = previous?.[0]?.requestId ?? uid();
    const nextAttempts = targetsToRun.map((target) => ({
      requestId,
      attemptId: uid(),
      target,
      snapshot,
      status: 'queued' as const,
    }));
    const retained = previous
      ? attemptsRef.current.filter((attempt) => !targetsToRun.some((target) => target.key === attempt.target.key))
      : [];
    const next = [...retained, ...nextAttempts];
    attemptsRef.current = next;
    setAttempts(next);
    setValidationError(null);
    for (const attempt of nextAttempts) {
      queueRef.current?.enqueue({
        key: attempt.target.key,
        attemptId: attempt.attemptId,
        payload: { attempt },
        execute: async (signal) => {
          const startedAt = Date.now();
          const request: JevRequest = { model: attempt.target.modelId, state: attempt.snapshot.state, questions: attempt.snapshot.questions };
          const response = attempt.target.kind === 'public'
            ? await postSystemOne(request, signal)
            : await invokeProviderModel(attempt.target.providerId, request, signal);
          return { response, durationMs: Date.now() - startedAt, finishedAt: Date.now() };
        },
      });
    }
  }, []);

  const runAll = () => {
    if (inputMode === 'form' && !formValid) { setValidationError(copy.formInvalid); return; }
    const runnableTargets = auth.isReadOnly ? targets.filter((target) => target.kind === 'public') : targets;
    if (runnableTargets.length === 0) {
      setValidationError(copy.noTargets);
      return;
    }
    let snapshot: Snapshot;
    try {
      snapshot = getSnapshot(stateJson, questionsJson);
    } catch (error) {
      setValidationError(error instanceof Error && error.message === 'questions' ? copy.invalidQuestions : copy.invalidState);
      return;
    }
    stopAll();
    enqueue(runnableTargets, snapshot);
  };

  const stopOne = (attempt: Attempt) => {
    queueRef.current?.cancel(attempt.target.key, attempt.attemptId);
  };

  function stopAll() {
    queueRef.current?.cancelAll();
  }

  const retryOne = (attempt: Attempt) => {
    if (auth.isReadOnly && attempt.target.kind === 'direct') return;
    enqueue([attempt.target], attempt.snapshot, [attempt]);
  };
  const currentFingerprint = useMemo(() => {
    try { return JSON.stringify([JSON.parse(stateJson), JSON.parse(questionsJson)]); }
    catch { return null; }
  }, [stateJson, questionsJson]);
  const visibleAttempts = auth.isReadOnly ? attempts.filter((attempt) => attempt.target.kind === 'public') : attempts;
  const hasStaleResult = visibleAttempts.some((attempt) => attempt.snapshot.fingerprint !== currentFingerprint);

  const uniquePublicModels = useMemo(() => [...new Set(publicModels.map((model) => model.id))], [publicModels]);
  const selectedProvider = providers.find((provider) => provider.id === providerToAdd);
  const providerModels = selectedProvider?.models ?? [];
  const providerName = (provider: AdminProvider) => {
    const label = provider.name || provider.account || provider.kind;
    return label === provider.id ? label : `${label} (${provider.id})`;
  };

  const addPublic = () => {
    if (!publicToAdd.trim()) return;
    const target: ComparisonTarget = { key: uid(), kind: 'public', modelId: publicToAdd.trim(), discovered: uniquePublicModels.includes(publicToAdd.trim()) };
    onTargetsChange([...targets, target]);
  };
  const addDirect = () => {
    if (auth.isReadOnly) return;
    const provider = selectedProvider;
    const modelId = upstreamModel.trim();
    if (!provider || !modelId) {
      setValidationError(copy.selectionRequired);
      return;
    }
    const target: ComparisonTarget = {
      key: uid(),
      kind: 'direct',
      providerId: provider.id,
      providerLabel: providerName(provider),
      modelId,
      discovered: providerModels.includes(modelId),
    };
    onTargetsChange([...targets, target]);
    setUpstreamModel('');
  };

  return (
    <section id="playground" className="fade-in overflow-hidden" style={cardStyle}>
      <header className="flex flex-wrap items-center justify-between gap-2 px-4 py-2" style={{ borderBottom: '1px solid var(--border)' }}>
        <div className="flex items-center gap-2">
          <strong style={{ color: 'var(--text)', fontSize: 'var(--text-sm)' }}>{copy.input}</strong>
          {inputsOpen && <>
            <button type="button" onClick={() => setInputMode('form')} aria-pressed={inputMode === 'form'} style={tabStyle(inputMode === 'form')}>{copy.form}</button>
            <button type="button" onClick={() => setInputMode('json')} aria-pressed={inputMode === 'json'} style={tabStyle(inputMode === 'json')}>{copy.json}</button>
          </>}
        </div>
        <button type="button" aria-expanded={inputsOpen} aria-controls="comparison-inputs" onClick={() => setInputsOpen((value) => !value)} style={{ ...buttonStyle, minHeight: 28, color: 'var(--text-muted)' }}>
          {inputsOpen ? copy.collapseInput : copy.expandInput}
        </button>
      </header>

      {inputsOpen ? <div id="comparison-inputs" className="comparison-inputs">
        <label className="comparison-editor comparison-state">
          <span className="font-mono text-xs" style={{ color: 'var(--text-muted)' }}>{copy.state}</span>
          <textarea value={stateJson} onChange={(event) => onStateChange(event.target.value)} spellCheck={false} rows={4} className="p-2.5 leading-relaxed" style={codeStyle} />
        </label>
        {inputMode === 'form' ? (
          <div className="comparison-editor comparison-questions">
            <span className="font-mono text-xs" style={{ color: 'var(--text-muted)' }}>{copy.questions}</span>
            <div className="comparison-question-scroll" role="region" aria-label={copy.questions} tabIndex={0}>
              <QuestionFormEditor questionsJson={questionsJson} onChange={onQuestionsChange} onValidityChange={setFormValid} />
            </div>
          </div>
        ) : (
          <label className="comparison-editor comparison-questions">
            <span className="font-mono text-xs" style={{ color: 'var(--text-muted)' }}>{copy.questions}</span>
            <textarea value={questionsJson} onChange={(event) => onQuestionsChange(event.target.value)} spellCheck={false} rows={7} className="p-2.5 leading-relaxed" style={codeStyle} />
          </label>
        )}
      </div> : <div id="comparison-inputs" className="comparison-input-summary" aria-live="polite">
        <span><span style={smallLabel}>{copy.state}</span> <code>{stateFields.length ? stateFields.join(' · ') : '—'}</code></span>
        <span><span style={smallLabel}>{copy.questions}</span> <code>{questionCount}</code></span>
      </div>}

      <div className="comparison-target-inputs flex flex-wrap items-end gap-3 px-4 py-3" data-entry-mode={entryMode} style={{ borderTop: '1px solid var(--border)', background: 'var(--surface-hover)' }}>
        {compactLayout && <div className="comparison-entry-tabs" aria-label={copy.publicEntry}>
          <button type="button" aria-pressed={entryMode === 'public'} onClick={() => setEntryMode('public')} style={tabStyle(entryMode === 'public')}>{copy.publicEntry}</button>
          {!auth.isReadOnly && <button type="button" aria-pressed={entryMode === 'direct'} onClick={() => setEntryMode('direct')} style={tabStyle(entryMode === 'direct')}>{copy.directEntry}</button>}
        </div>}
        <div className="comparison-public-entry flex min-w-0 basis-60 grow flex-col gap-1.5">
          <span className="font-semibold" style={{ color: 'var(--text)', fontSize: 'var(--text-sm)' }}>{copy.publicEntry}</span>
          <span className="comparison-entry-hint" style={smallLabel}>{copy.publicHint}{publicModelStatus === 'ready' ? ` · ${copy.discovered}` : ''}</span>
          <div className="flex gap-2">
            <input aria-label={copy.publicEntry} list="playground-public-models" value={publicToAdd} onChange={(event) => setPublicToAdd(event.target.value)} placeholder={copy.modelId} className="h-8 min-w-0 flex-1 px-2" style={{ ...codeStyle, fontFamily: 'var(--font-mono)' }} />
            <datalist id="playground-public-models">{uniquePublicModels.map((modelId) => <option key={modelId} value={modelId} />)}</datalist>
            <button type="button" onClick={addPublic} disabled={!publicToAdd.trim()} style={buttonStyle}>{copy.add}</button>
          </div>
          {publicModelStatus === 'loading' && <span role="status" style={smallLabel}>{copy.modelsLoading}</span>}
          {publicModelStatus === 'empty' && <span style={smallLabel}>{copy.noPublicModels}</span>}
          {publicModelStatus === 'unauthorized' && <div role="alert" className="text-xs" style={{ color: 'var(--danger)' }}>
            <p>{copy.modelsAuthRequired}</p>
            <a href="#/dashboard" className="mt-1 inline-block underline" style={{ color: 'var(--accent)' }}>{copy.openDashboard}</a>
            <p className="mt-1" style={smallLabel}>{copy.callerTokenHelp}</p>
          </div>}
          {publicModelStatus === 'forbidden' && <div role="alert" className="text-xs" style={{ color: 'var(--danger)' }}>
            <p>{copy.modelsForbidden}</p>
            <a href="#/dashboard" className="mt-1 inline-block underline" style={{ color: 'var(--accent)' }}>{copy.openDashboard}</a>
          </div>}
          {publicModelStatus === 'network-error' && <span role="alert" className="text-xs" style={{ color: 'var(--danger)' }}>{copy.modelsNetworkError}</span>}
          {publicModelStatus === 'request-error' && <span role="alert" className="text-xs" style={{ color: 'var(--danger)' }}>{copy.modelsRequestError}</span>}
        </div>
        {!auth.isReadOnly && <div className="comparison-direct-entry flex min-w-0 basis-72 grow-[1.3] flex-col gap-1.5">
          <span className="font-semibold" style={{ color: 'var(--text)', fontSize: 'var(--text-sm)' }}>{copy.directEntry}</span>
          <span className="comparison-entry-hint" style={smallLabel}>{copy.directHint}</span>
          <div className="flex flex-wrap gap-2">
            <select aria-label={copy.providerAccount} value={providerToAdd} onChange={(event) => { setProviderToAdd(event.target.value); setUpstreamModel(''); }} className="h-8 min-w-[160px] flex-1 px-2" style={codeStyle} disabled={providersLoading}>
              <option value="">{copy.chooseProvider}</option>
              {providers.map((provider) => <option key={provider.id} value={provider.id} disabled={!provider.enabled}>
                {providerName(provider)}{!provider.enabled ? ` · ${copy.disabled}` : !provider.api_key_set ? ` · ${copy.noKey}` : ''}
              </option>)}
            </select>
            <input aria-label={copy.modelId} list="playground-upstream-models" value={upstreamModel} onChange={(event) => setUpstreamModel(event.target.value)} placeholder={copy.chooseModel} className="h-8 min-w-[170px] flex-[1.2] px-2" style={codeStyle} />
            <datalist id="playground-upstream-models">{providerModels.map((modelId) => <option key={modelId} value={modelId} />)}</datalist>
            <button type="button" onClick={addDirect} disabled={!selectedProvider || !selectedProvider.enabled || !upstreamModel.trim()} style={buttonStyle}>{copy.add}</button>
          </div>
          {providersError && <span role="alert" style={{ ...smallLabel, color: 'var(--danger)' }}>{copy.providerLoadError}</span>}
          {!providersLoading && providers.length === 0 && <span style={smallLabel}>{copy.noProviders}</span>}
        </div>}
      </div>

      <div className="flex flex-wrap items-center gap-2 px-4 py-2.5" style={{ borderTop: '1px solid var(--border)' }}>
        {targets.filter((target) => !auth.isReadOnly || target.kind === 'public').map((target) => <span key={target.key} className="inline-flex max-w-full items-center gap-2 border px-2 py-1" style={{ borderColor: 'var(--border)', borderRadius: 'var(--radius)', background: 'var(--surface-hover)', color: 'var(--text)', fontSize: 'var(--text-xs)' }}>
          <span className="shrink-0" style={{ color: 'var(--text-muted)' }}>{target.kind === 'public' ? copy.publicEntry : copy.directEntry}</span>
          <span className="min-w-0 max-w-[22rem] truncate font-mono" title={targetLabel(target, copy)}>{target.kind === 'public' ? target.modelId : targetLabel(target, copy)}</span>
          <button type="button" aria-label={`${copy.remove} ${targetLabel(target, copy)}`} onClick={() => onTargetsChange(targets.filter((item) => item.key !== target.key))} style={{ color: 'var(--text-muted)' }}>×</button>
        </span>)}
        <span className="ml-auto flex gap-2">
          <button type="button" onClick={runAll} disabled={targets.length === 0 || (inputMode === 'form' && !formValid)} className="font-semibold" style={{ ...buttonStyle, borderColor: 'var(--accent)', background: 'var(--accent)', color: '#fff' }}>{copy.runAll}{runningCount > 0 ? ` · ${runningCount}` : ''}</button>
          <button type="button" onClick={stopAll} disabled={runningCount === 0} style={buttonStyle}>{copy.cancelAll}</button>
        </span>
      </div>
      {validationError && <div role="alert" className="px-4 py-2 text-sm" style={{ color: 'var(--danger)', borderTop: '1px solid var(--border)' }}>{validationError}</div>}
      {hasStaleResult && <div role="status" className="px-4 py-2 text-sm" style={{ color: 'var(--warning)', borderTop: '1px solid var(--border)' }}>{copy.inputChanged}</div>}

      <div className="grid border-t border-border" style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(min(100%, 320px), 1fr))' }}>
        {visibleAttempts.map((attempt) => <AttemptCard key={attempt.target.key} attempt={attempt} copy={copy} onCancel={() => stopOne(attempt)} onRetry={() => retryOne(attempt)} stale={attempt.snapshot.fingerprint !== currentFingerprint} />)}
        {visibleAttempts.length === 0 && <div className="px-4 py-8 text-center text-sm" style={{ gridColumn: '1 / -1', color: 'var(--text-subtle)' }}>{targets.length === 0 ? copy.noTargets : copy.ready}</div>}
      </div>
    </section>
  );
}

function tabStyle(active: boolean): React.CSSProperties {
  return { minHeight: 28, padding: '0 9px', border: `1px solid ${active ? 'var(--accent)' : 'transparent'}`, borderRadius: 'var(--radius)', background: active ? 'var(--accent)' : 'transparent', color: active ? '#fff' : 'var(--text-muted)', fontSize: 'var(--text-sm)' };
}

function formatError(error: Error, copy: PlaygroundCopy): string {
  let message = error.message;
  if (error instanceof SystemOneError) {
    const detail = error.upstream ? ` · ${error.upstream}` : '';
    message = `${error.status}${detail}: ${error.message}`;
  }
  const requestId = error instanceof SystemOneError || error instanceof AdminApiError ? error.requestId : undefined;
  return requestId ? `${message} · ${copy.requestId}: ${requestId}` : message;
}

function AttemptCard({ attempt, copy, onCancel, onRetry, stale }: { attempt: Attempt; copy: PlaygroundCopy; onCancel: () => void; onRetry: () => void; stale: boolean }) {
  const statusText: Record<Status, string> = { idle: copy.waiting, queued: copy.queued, loading: copy.running, ok: copy.success, error: copy.failed, cancelled: copy.cancelled };
  const statusColor = attempt.status === 'ok' ? 'var(--success)' : attempt.status === 'error' ? 'var(--danger)' : attempt.status === 'loading' || attempt.status === 'queued' ? 'var(--warning)' : 'var(--text-subtle)';
  const target = attempt.target;
  const displayTarget = target.kind === 'public' ? `${target.modelId}${target.discovered ? '' : ` · ${copy.manual}`}` : `${target.providerLabel} · ${target.modelId}${target.discovered ? '' : ` · ${copy.manual}`}`;
  const answers = attempt.response?.answers;
  const responseModel = attempt.response?.model;
  const responseRequestId = attempt.response?.request_id;
  const requestId = typeof responseRequestId === 'string' ? responseRequestId : null;
  return (
    <article className="flex min-w-0 flex-col border-b border-r border-border" style={{ background: 'var(--surface)' }}>
      <header className="flex min-w-0 items-start justify-between gap-3 px-4 py-3" style={{ borderBottom: '1px solid var(--border)' }}>
        <div className="min-w-0">
          <div className="mb-1" style={{ ...smallLabel, color: 'var(--accent)' }}>{target.kind === 'public' ? copy.publicEntry : copy.directEntry}</div>
          <div className="break-all font-mono text-sm font-semibold" style={{ color: 'var(--text)' }}>{displayTarget}</div>
          {stale && <div className="mt-1 text-xs" style={{ color: 'var(--warning)' }}>{copy.previousInput}</div>}
        </div>
        <div className="shrink-0 text-right text-xs" style={{ color: statusColor }}>
          <div>{statusText[attempt.status]}</div>
          {attempt.durationMs !== undefined && <div className="mt-1 tabular" style={{ color: 'var(--text-subtle)' }}>{attempt.durationMs}ms</div>}
        </div>
      </header>
      <div className="flex-1 space-y-3 px-4 py-3">
        {attempt.error && <div className="whitespace-pre-wrap break-words border p-2.5 text-xs" style={{ borderColor: 'var(--danger)', background: 'var(--danger-bg)', color: 'var(--danger)' }}>{attempt.error}</div>}
        {attempt.status === 'ok' && attempt.response && <>
          {typeof responseModel === 'string' && <div className="text-xs" style={{ color: 'var(--text-muted)' }}>{copy.actualModel}: <code className="break-all">{responseModel}</code>{responseModel !== target.modelId && <span className="ml-2" style={{ color: target.kind === 'public' ? 'var(--text-muted)' : 'var(--warning)' }}>{target.kind === 'public' ? copy.routedModel : copy.targetMismatch}</span>}</div>}
          {requestId && <div className="text-xs" style={{ color: 'var(--text-muted)' }}>{copy.requestId}: <code className="break-all">{requestId}</code></div>}
          {(target.kind === 'public' || attempt.response.route_trace !== undefined) && <RouteTrace value={attempt.response.route_trace} copy={copy} />}
          <AnswerList answers={answers ?? {}} copy={copy} />
          <Metering response={attempt.response} snapshot={attempt.snapshot} durationMs={attempt.durationMs} copy={copy} />
          <details>
            <summary className="cursor-pointer text-xs" style={{ color: 'var(--text-muted)' }}>{copy.rawResponse}</summary>
            <pre className="mt-2 max-h-64 overflow-auto whitespace-pre-wrap break-words p-2" style={codeStyle}>{JSON.stringify(attempt.response, null, 2)}</pre>
          </details>
        </>}
        {attempt.status === 'idle' && <span className="text-sm" style={{ color: 'var(--text-subtle)' }}>{copy.waiting}</span>}
      </div>
      <footer className="flex justify-end gap-2 px-4 py-2" style={{ borderTop: '1px solid var(--border)' }}>
        {(attempt.status === 'queued' || attempt.status === 'loading') && <button type="button" onClick={onCancel} style={buttonStyle}>{copy.cancel}</button>}
        {(attempt.status === 'error' || attempt.status === 'cancelled' || attempt.status === 'ok') && <button type="button" onClick={onRetry} style={buttonStyle}>{copy.retry}</button>}
      </footer>
    </article>
  );
}

function AnswerList({ answers, copy }: { answers: JevResponse['answers']; copy: PlaygroundCopy }) {
  const rows = Object.entries(answers ?? {});
  if (rows.length === 0) return null;
  return <div className="space-y-2">
    <div className="text-xs font-semibold" style={{ color: 'var(--text-muted)' }}>{copy.answers}</div>
    {rows.map(([qid, answer]) => {
      if (answer.type === 'choice') return <div key={qid} className="flex flex-wrap items-baseline gap-2 text-sm">
        <code style={{ color: 'var(--text-subtle)' }}>{qid}</code><span style={{ color: 'var(--text)' }}>{answer.choice}</span><span className="text-xs" style={{ color: 'var(--text-muted)' }}>{copy.confidence} {fmt(answer.confidence)}</span><Distribution probabilities={answer.probabilities} highlight={answer.choice} />
      </div>;
      if (answer.type === 'score') return <div key={qid} className="flex flex-wrap items-baseline gap-2 text-sm">
        <code style={{ color: 'var(--text-subtle)' }}>{qid}</code><span style={{ color: 'var(--text)' }}>{fmt(answer.score)}</span><span className="text-xs" style={{ color: 'var(--text-muted)' }}>{copy.confidence} {fmt(answer.confidence)}</span><Distribution probabilities={answer.probabilities} highlight={String(answer.score)} />
      </div>;
      const probability = answer.probability;
      const noul = answer.noul;
      return <div key={qid} className="flex flex-wrap items-baseline gap-2 text-sm">
        <code style={{ color: 'var(--text-subtle)' }}>{qid}</code><span className="text-xs" style={{ color: 'var(--text-muted)' }}>{answer.type}</span>
        <span style={{ color: 'var(--text)' }}>{probability !== null && probability !== undefined ? `prob ${fmt(probability)}` : ''}{probability != null && noul != null ? ' · ' : ''}{noul !== null && noul !== undefined ? `noul ${fmt(noul)}` : ''}{probability == null && noul == null ? '—' : ''}</span>
      </div>;
    })}
  </div>;
}

function RouteTrace({ value, copy }: { value: unknown; copy: PlaygroundCopy }) {
  const record = (item: unknown): Record<string, unknown> | null => item !== null && typeof item === 'object' && !Array.isArray(item) ? item as Record<string, unknown> : null;
  const text = (item: unknown) => typeof item === 'string' ? item : '';
  const trace = record(value);
  if (!trace) return <p className="text-xs" style={{ color: 'var(--text-muted)' }}>{copy.traceUnavailable}</p>;
  const hops = Array.isArray(trace.selected_hops) ? trace.selected_hops.filter((hop): hop is string => typeof hop === 'string') : [];
  const direct = trace.kind === 'direct_upstream';
  const provider = text(trace.selected_provider) || text(trace.provider_config_id);
  const model = text(trace.selected_model);
  if (provider && hops[hops.length - 1] === provider) hops.pop();
  const selectedPath = direct
    ? [provider, model].filter(Boolean)
    : [text(trace.requested_model), ...hops, [provider, model].filter(Boolean).join(' / ')].filter(Boolean);
  const attempts = Array.isArray(trace.attempts) ? trace.attempts.map(record).filter((attempt): attempt is Record<string, unknown> => attempt !== null) : [];
  const shadow = record(trace.shadow);
  return <div className="space-y-1 text-xs" style={{ color: 'var(--text-muted)' }}>
    <div>{copy.executionPath}: <code className="break-all">{selectedPath.length ? selectedPath.join(' → ') : copy.traceUnavailable}</code></div>
    {typeof trace.strategy === 'string' && <div>{copy.strategy}: <code>{trace.strategy}</code></div>}
    {shadow?.dispatched === true && <div className="break-words">{copy.shadowDispatched}: <code>{text(shadow.target)} / {text(shadow.model)}</code></div>}
    {attempts.length > 0 && <details>
      <summary className="cursor-pointer">{copy.dispatchedAttempts} · {attempts.length}</summary>
      <ol className="mt-1 list-inside list-decimal space-y-1">
        {attempts.map((attempt, index) => <li key={index} className="break-all"><code>{[text(attempt.provider_id), text(attempt.upstream_model)].filter(Boolean).join(' / ') || '—'}</code></li>)}
      </ol>
    </details>}
  </div>;
}

function Distribution({ probabilities, highlight }: { probabilities: Record<string, number>; highlight: string }) {
  const entries = Object.entries(probabilities ?? {}).filter(([, value]) => Number.isFinite(value)).sort((a, b) => b[1] - a[1]);
  return <span className="flex min-w-[100px] flex-1 flex-wrap gap-x-2 text-xs" style={{ color: 'var(--text-subtle)' }}>
    {entries.map(([key, value]) => <span key={key} className={key === highlight ? 'font-semibold' : ''} style={{ color: key === highlight ? 'var(--text)' : undefined }}>{key} {fmt(value)}</span>)}
  </span>;
}

function Metering({ response, snapshot, durationMs, copy }: { response: JevResponse; snapshot: Snapshot; durationMs?: number; copy: PlaygroundCopy }) {
  const usage = response.usage;
  const hasUsage = usage && [usage.input_tokens, usage.output_tokens, usage.reasoning_tokens].some((value) => typeof value === 'number');
  const estimatedInput = Math.ceil((snapshot.stateJson.length + snapshot.questionsJson.length) / 4);
  const latency = response.latency_ms;
  const cost = response.cost_usd;
  const trace = response.route_trace;
  const gatewayLatency = trace && typeof trace === 'object' && !Array.isArray(trace) && typeof trace.gateway_latency_ms === 'number' ? trace.gateway_latency_ms : null;
  return <div className="flex flex-wrap gap-x-3 gap-y-1 border-t pt-2 text-xs" style={{ borderColor: 'var(--border)', color: 'var(--text-muted)' }}>
    {hasUsage ? <>
      <span>{copy.tokens}</span><span>{copy.in}: {usage.input_tokens ?? '—'}</span><span>{copy.out}: {usage.output_tokens ?? '—'}</span><span>{copy.reasoning}: {usage.reasoning_tokens ?? '—'}</span>
    </> : <>
      <span>{copy.tokens} · {copy.estimate}</span><span>{copy.in}: ~{estimatedInput}</span><span>{copy.out}: —</span>
    </>}
    <span>{copy.clientLatency}: {durationMs ?? '—'}ms</span>
    <span>{copy.gatewayLatency}: {gatewayLatency ?? '—'}{gatewayLatency != null ? 'ms' : ''}</span>
    <span>{copy.serverLatency}: {latency ?? '—'}{latency != null ? 'ms' : ''}</span>
    <span>{copy.upstreamCalls}: {response.upstream_calls ?? '—'}</span>
    <span>{copy.cost}: {typeof cost === 'number' ? formatCost(cost) : copy.costUnknown}</span>
    {(response.upstream_calls ?? 0) > 1 && <p className="basis-full pt-1">{copy.meteringScope}</p>}
  </div>;
}

function fmt(value: number): string { return Number.isInteger(value) ? String(value) : String(Number(value.toFixed(3))); }
function formatCost(value: number): string { return value === 0 ? '0' : value.toFixed(6).replace(/0+$/, '').replace(/\.$/, '') || value.toExponential(2); }
