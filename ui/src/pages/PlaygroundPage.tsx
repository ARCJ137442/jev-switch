import { useEffect, useRef, useState } from 'react';
import { getBase, fetchModels, type ModelEntry } from '../api';
import { listProviders, type AdminProvider } from '../api/admin';
import { ComparisonPanel, type ComparisonTarget } from '../components/playground/ComparisonPanel';
import { ExampleChips } from '../components/playground/ExampleChips';
import { useServerStatus } from '../app/Shell';
import { EXAMPLES, type ExamplePayload } from '../examples';
import { useI18n, type MessageKey } from '../i18n';
import { useAuth } from '../auth/AuthContext';

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
  const [stateJson, setStateJson] = useState(() => JSON.stringify(EXAMPLES[0].payload.state, null, 2));
  const [questionsJson, setQuestionsJson] = useState(() => JSON.stringify(EXAMPLES[0].payload.questions, null, 2));
  const [activeExampleId, setActiveExampleId] = useState<string>(EXAMPLES[0].id);
  const [targets, setTargets] = useState<ComparisonTarget[]>([]);
  const userEditedTargets = useRef(false);
  const targetChange = (next: ComparisonTarget[]) => {
    userEditedTargets.current = true;
    setTargets(next);
  };

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
          <ExampleChips examples={EXAMPLES} onPick={onPickExample} activeId={activeExampleId} />
          <div className="playground-daemon-status flex items-center gap-2 text-xs" style={{ color: 'var(--text-subtle)' }}>
            <span className="inline-block h-2 w-2 rounded-full" style={{ background: server.status === 'ok' ? 'var(--success)' : 'var(--text-subtle)' }} />
            {server.status === 'ok' ? t('cmp.daemonConnected' as MessageKey) : t('cmp.daemonUnavailable' as MessageKey)}
          </div>
        </section>
      </div>
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
