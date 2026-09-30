import { useEffect, useMemo, useRef, useState } from 'react';
import { Download, Upload } from 'lucide-react';
import { getBase, fetchModels, type ModelEntry } from '../api';
import { listProviders, type AdminProvider } from '../api/admin';
import { ComparisonPanel, type ComparisonTarget } from '../components/playground/ComparisonPanel';
import { ExampleChips } from '../components/playground/ExampleChips';
import { useServerStatus } from '../app/Shell';
import { EXAMPLES, type ExamplePayload } from '../examples';
import { useI18n, type MessageKey } from '../i18n';
import { useAuth } from '../auth/AuthContext';
import type { JevRequest } from '../api';

const PLAYGROUND_STORAGE_KEY = 'jev-playground-workspace-v1';
const EXAMPLE_FILE_MAX_BYTES = 1024 * 1024;

interface SavedPlaygroundWorkspace {
  version: 1;
  stateJson: string;
  questionsJson: string;
  activeExampleId: string;
  targets: ComparisonTarget[];
  selectedBatchExampleIds: string[];
  customExamples: ExamplePayload[];
}

function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function parseExample(value: unknown): ExamplePayload | null {
  if (!isObject(value) || typeof value.id !== 'string' || typeof value.label !== 'string' || typeof value.description !== 'string' || !isObject(value.payload)) return null;
  const payload = value.payload;
  if (typeof payload.model !== 'string' || !isObject(payload.state) || !isObject(payload.questions) || Object.keys(payload.questions).length === 0) return null;
  return { id: value.id, label: value.label, description: value.description, payload: payload as unknown as JevRequest };
}

function readWorkspace(): SavedPlaygroundWorkspace | null {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(PLAYGROUND_STORAGE_KEY) ?? 'null');
    if (!isObject(stored) || stored.version !== 1) return null;
    const targets = Array.isArray(stored.targets) ? stored.targets.filter((target): target is ComparisonTarget => {
      if (!isObject(target) || typeof target.key !== 'string' || typeof target.modelId !== 'string') return false;
      if (target.kind === 'public') return true;
      return target.kind === 'direct' && typeof target.providerId === 'string' && typeof target.providerLabel === 'string' && typeof target.discovered === 'boolean';
    }) : [];
    const customExamples = Array.isArray(stored.customExamples) ? stored.customExamples.map(parseExample).filter((example): example is ExamplePayload => example !== null).slice(0, 100) : [];
    return {
      version: 1,
      stateJson: typeof stored.stateJson === 'string' ? stored.stateJson : JSON.stringify(EXAMPLES[0].payload.state, null, 2),
      questionsJson: typeof stored.questionsJson === 'string' ? stored.questionsJson : JSON.stringify(EXAMPLES[0].payload.questions, null, 2),
      activeExampleId: typeof stored.activeExampleId === 'string' ? stored.activeExampleId : EXAMPLES[0].id,
      targets,
      selectedBatchExampleIds: Array.isArray(stored.selectedBatchExampleIds) ? stored.selectedBatchExampleIds.filter((id): id is string => typeof id === 'string') : [],
      customExamples,
    };
  } catch {
    return null;
  }
}

function downloadExamples(examples: ExamplePayload[]) {
  const blob = new Blob([JSON.stringify({ schema_version: 1, examples }, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = 'jev-playground-examples.json';
  anchor.click();
  URL.revokeObjectURL(url);
}

type PublicModelStatus = 'loading' | 'ready' | 'empty' | 'unauthorized' | 'forbidden' | 'network-error' | 'request-error';

function classifyModelDiscoveryError(error: unknown): PublicModelStatus {
  const message = error instanceof Error ? error.message : '';
  const status = /^models\s+(\d{3})$/.exec(message)?.[1];
  if (status === '401') return 'unauthorized';
  if (status === '403') return 'forbidden';
  if (status) return 'request-error';
  return error instanceof TypeError ? 'network-error' : 'request-error';
}

export function PlaygroundPage() {
  const { t } = useI18n();
  const auth = useAuth();
  const server = useServerStatus();
  const [publicModels, setPublicModels] = useState<ModelEntry[]>([]);
  const [publicModelStatus, setPublicModelStatus] = useState<PublicModelStatus>('loading');
  const [providers, setProviders] = useState<AdminProvider[]>([]);
  const [providersLoading, setProvidersLoading] = useState(false);
  const [providersError, setProvidersError] = useState(false);
  const [showCurl, setShowCurl] = useState(false);
  const [savedWorkspace] = useState(readWorkspace);
  const [customExamples, setCustomExamples] = useState<ExamplePayload[]>(() => savedWorkspace?.customExamples ?? []);
  const examples = useMemo(() => [...EXAMPLES, ...customExamples], [customExamples]);
  const [stateJson, setStateJson] = useState(() => savedWorkspace?.stateJson ?? JSON.stringify(EXAMPLES[0].payload.state, null, 2));
  const [questionsJson, setQuestionsJson] = useState(() => savedWorkspace?.questionsJson ?? JSON.stringify(EXAMPLES[0].payload.questions, null, 2));
  const [activeExampleId, setActiveExampleId] = useState<string>(() => savedWorkspace?.activeExampleId ?? EXAMPLES[0].id);
  const [targets, setTargets] = useState<ComparisonTarget[]>(() => savedWorkspace?.targets ?? []);
  const [selectedBatchExampleIds, setSelectedBatchExampleIds] = useState<string[]>(() => savedWorkspace?.selectedBatchExampleIds ?? []);
  const [exampleError, setExampleError] = useState<string | null>(null);
  const importInput = useRef<HTMLInputElement>(null);
  const userEditedTargets = useRef(savedWorkspace !== null);
  const targetChange = (next: ComparisonTarget[]) => {
    userEditedTargets.current = true;
    setTargets(next);
  };

  useEffect(() => {
    try {
      localStorage.setItem(PLAYGROUND_STORAGE_KEY, JSON.stringify({ version: 1, stateJson, questionsJson, activeExampleId, targets, selectedBatchExampleIds, customExamples } satisfies SavedPlaygroundWorkspace));
    } catch {
      setExampleError(t('pg.localSaveFailed' as MessageKey));
    }
  }, [activeExampleId, customExamples, questionsJson, selectedBatchExampleIds, stateJson, targets, t]);

  useEffect(() => {
    if (server.status !== 'ok') {
      setPublicModelStatus(server.status === 'error' ? 'network-error' : 'loading');
      return;
    }
    let cancelled = false;
    setPublicModelStatus('loading');
    fetchModels().then((models) => {
      if (!cancelled) {
        const discovered = Array.isArray(models?.data) ? models.data : [];
        setPublicModels(discovered);
        setPublicModelStatus(discovered.length > 0 ? 'ready' : 'empty');
        if (!userEditedTargets.current && discovered.length > 0) {
          setTargets(discovered.slice(0, 2).map((entry) => ({ key: `initial-${entry.id}`, kind: 'public', modelId: entry.id, discovered: true })));
        }
      }
    }).catch((error: unknown) => {
      if (!cancelled) {
        setPublicModels([]);
        setPublicModelStatus(classifyModelDiscoveryError(error));
      }
    });
    if (auth.isReadOnly) {
      setProviders([]);
      setProvidersLoading(false);
      setProvidersError(false);
    } else {
      setProvidersLoading(true);
      setProvidersError(false);
      listProviders().then((result) => {
        if (!cancelled) setProviders(Array.isArray(result?.providers) ? result.providers : []);
      }).catch(() => {
        if (!cancelled) setProvidersError(true);
      }).finally(() => {
        if (!cancelled) setProvidersLoading(false);
      });
    }
    return () => { cancelled = true; };
  }, [server.status, auth.isReadOnly]);

  const curlModel = targets.find((target) => target.kind === 'public')?.modelId ?? 'your-public-model-id';
  const onPickExample = (example: ExamplePayload) => {
    setActiveExampleId(example.id);
    setStateJson(JSON.stringify(example.payload.state, null, 2));
    setQuestionsJson(JSON.stringify(example.payload.questions, null, 2));
  };
  const batchCases = examples.filter((example) => selectedBatchExampleIds.includes(example.id)).map((example) => ({ id: example.id, label: example.label, stateJson: JSON.stringify(example.payload.state), questionsJson: JSON.stringify(example.payload.questions) }));
  const importExamples = async (file?: File) => {
    if (!file) return;
    if (file.size > EXAMPLE_FILE_MAX_BYTES) { setExampleError(t('pg.importTooLarge' as MessageKey)); return; }
    try {
      const parsed: unknown = JSON.parse(await file.text());
      const items = Array.isArray(parsed) ? parsed : isObject(parsed) && parsed.schema_version === 1 && Array.isArray(parsed.examples) ? parsed.examples : null;
      if (!items) throw new Error(t('pg.importInvalid' as MessageKey));
      const imported = items.map(parseExample);
      if (imported.some((example) => example === null)) throw new Error(t('pg.importInvalid' as MessageKey));
      const existing = new Set(examples.map((example) => example.id));
      if (imported.some((example) => existing.has(example!.id))) throw new Error(t('pg.importDuplicate' as MessageKey));
      setCustomExamples((current) => [...current, ...(imported as ExamplePayload[])]);
      setExampleError(null);
    } catch (error) {
      setExampleError(error instanceof Error ? error.message : String(error));
    } finally {
      if (importInput.current) importInput.current.value = '';
    }
  };
  const curlBase = getBase() || window.location.origin;

  const card: React.CSSProperties = {
    background: 'var(--surface)',
    border: '1px solid var(--border)',
    borderRadius: 'var(--radius)',
  };

  return (
    <div className="playground-page w-full min-w-0 px-4 sm:px-6 lg:px-8">
      <div className="playground-topbar">
        <div className="playground-page-title mb-4 flex items-center gap-2">
          <h1 className="font-semibold" style={{ fontSize: 'var(--text-2xl)', color: 'var(--text)' }}>{t('shell.navPlayground')}</h1>
          <span role="note" tabIndex={0} aria-label={t('pg.heroLead')} title={t('pg.heroLead')} className="inline-flex h-5 w-5 cursor-help items-center justify-center" style={{ borderRadius: '50%', border: '1px solid var(--border)', background: 'var(--surface)', color: 'var(--text-muted)', fontSize: 'var(--text-xs)' }}>?</span>
        </div>
        <section className="playground-examples-row mb-3 flex flex-wrap items-center justify-between gap-3" aria-label={t('pg.examples')}>
          <div className="flex min-w-0 flex-1 flex-wrap items-center gap-2">
            <ExampleChips examples={examples} onPick={onPickExample} activeId={activeExampleId} />
            <input ref={importInput} type="file" accept="application/json,.json" className="sr-only" onChange={(event) => void importExamples(event.currentTarget.files?.[0])}/>
            <button type="button" className="inline-flex h-9 items-center gap-1.5 border px-2 text-xs" style={{ borderColor: 'var(--border)', borderRadius: 'var(--radius)', color: 'var(--text-muted)' }} title={t('pg.importExamples' as MessageKey)} aria-label={t('pg.importExamples' as MessageKey)} onClick={() => importInput.current?.click()}><Upload size={14}/>{t('pg.importExamples' as MessageKey)}</button>
            <button type="button" className="inline-flex h-9 items-center gap-1.5 border px-2 text-xs" style={{ borderColor: 'var(--border)', borderRadius: 'var(--radius)', color: 'var(--text-muted)' }} title={t('pg.exportExamples' as MessageKey)} aria-label={t('pg.exportExamples' as MessageKey)} onClick={() => downloadExamples(examples)}><Download size={14}/>{t('pg.exportExamples' as MessageKey)}</button>
          </div>
          <div className="playground-daemon-status flex items-center gap-2 text-xs" style={{ color: 'var(--text-subtle)' }}>
            <span className="inline-block h-2 w-2 rounded-full" style={{ background: server.status === 'ok' ? 'var(--success)' : 'var(--text-subtle)' }} />
            {server.status === 'ok' ? t('cmp.daemonConnected' as MessageKey) : t('cmp.daemonUnavailable' as MessageKey)}
          </div>
        </section>
      </div>
      <details className="mb-3 border px-4 py-3" style={{ borderColor: 'var(--border)', borderRadius: 'var(--radius)', background: 'var(--surface)' }}>
        <summary className="cursor-pointer text-sm font-medium">{t('pg.batchExamples' as MessageKey)}{selectedBatchExampleIds.length > 0 ? ` · ${selectedBatchExampleIds.length}` : ''}</summary>
        <div className="mt-3 grid gap-2 sm:grid-cols-2 lg:grid-cols-3">{examples.map((example) => <label key={example.id} className="flex min-w-0 items-center gap-2 text-sm"><input type="checkbox" checked={selectedBatchExampleIds.includes(example.id)} onChange={(event) => setSelectedBatchExampleIds((current) => event.target.checked ? [...current, example.id] : current.filter((id) => id !== example.id))}/><span className="truncate" title={example.description}>{example.label}</span></label>)}</div>
        <p className="mt-2 text-xs" style={{ color: 'var(--text-muted)' }}>{t('pg.batchCountHint' as MessageKey, { models: targets.length, examples: selectedBatchExampleIds.length || 1 })}</p>
      </details>
      {exampleError && <p role="alert" className="mb-3 text-sm" style={{ color: 'var(--danger)' }}>{exampleError}</p>}
      {auth.isReadOnly && <p className="mb-4 rounded-md px-3 py-2 text-sm" style={{ color: 'var(--text-muted)', background: 'var(--surface-hover)' }}>{t('access.readonlyPlaygroundHint' as MessageKey)}</p>}

      <ComparisonPanel
        stateJson={stateJson}
        questionsJson={questionsJson}
        onStateChange={setStateJson}
        onQuestionsChange={setQuestionsJson}
        publicModels={publicModels}
        publicModelStatus={publicModelStatus}
        providers={providers}
        providersLoading={providersLoading}
        providersError={providersError}
        targets={targets}
        onTargetsChange={targetChange}
        batchCases={batchCases}
      />

      <section className="mt-3 overflow-hidden px-4 py-3" style={card}>
        <div className="flex flex-wrap items-baseline justify-between gap-3">
          <h2 className="font-semibold" style={{ fontSize: 'var(--text-sm)', color: 'var(--text)' }}>{t('pg.quickStart')}</h2>
          <button type="button" onClick={() => setShowCurl((value) => !value)} aria-expanded={showCurl} style={{ border: 0, background: 'transparent', color: 'var(--accent)', cursor: 'pointer', fontSize: 'var(--text-sm)' }}>
            {showCurl ? t('pg.hideCurl') : t('pg.showCurl')}
          </button>
        </div>
        {showCurl && <pre className="mt-3 overflow-x-auto whitespace-pre-wrap p-3" style={{ border: '1px solid var(--border)', borderRadius: 'var(--radius)', background: 'var(--surface-hover)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)', color: 'var(--text)' }}>{`curl -X POST ${curlBase}/v1/systemone \\
  -H "Content-Type: application/json" \\
  -d '{
    "model": "${curlModel}",
    "state": { "test": true },
    "questions": { "q": { "type": "noul", "instructions": "is this a test?", "criteria": { "true": "yes", "false": "no" } } }
  }'`}</pre>}
      </section>
    </div>
  );
}
